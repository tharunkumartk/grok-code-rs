use crate::events::{AppEvent, EventSender};
use crate::tools::types::*;
use serde_json::{json, Value};
use std::path::Path;
use std::time::Instant;
use walkdir::WalkDir;

/// LLM-powered tool executor
pub struct LlmExecutor {
    event_sender: EventSender,
    max_output_size: usize,
}

impl LlmExecutor {
    pub fn new(event_sender: EventSender, max_output_size: usize) -> Self {
        Self {
            event_sender,
            max_output_size,
        }
    }

    /// Truncate a JSON value if it exceeds the maximum output size
    fn truncate_result(&self, result: Value) -> Value {
        let json_str = serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string());

        if json_str.len() <= self.max_output_size {
            result
        } else {
            // Create a truncated result with a clear message
            serde_json::json!({
                "truncated": true,
                "original_size_bytes": json_str.len(),
                "max_allowed_bytes": self.max_output_size,
                "message": "The tool output was too large and has been truncated. The rest of the output was too long.",
                "note": "Output exceeded the maximum size limit to prevent excessive token usage in the conversation."
            })
        }
    }

    pub async fn execute_large_context_fetch(&self, id: String, args: Value) -> Result<(), String> {
        let _result = self.execute_large_context_fetch_with_result(id, args).await?;
        Ok(())
    }

    pub async fn execute_large_context_fetch_with_result(&self, id: String, args: Value) -> Result<Value, String> {
        let args: LargeContextFetchArgs = serde_json::from_value(args)
            .map_err(|e| format!("Invalid LargeContextFetch arguments: {}", e))?;

        let start = Instant::now();

        // Send progress event
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: "Gathering code files for analysis...".to_string(),
        }).map_err(|e| format!("Failed to send progress event: {}", e))?;

        // Step 1: Gather all code files
        let base_path = args.base_path.as_deref().unwrap_or(".");
        let max_files = args.max_files.unwrap_or(200); // Default limit to prevent overwhelming the LLM
        
        let code_files = self.gather_code_files(
            base_path,
            &args.include_extensions,
            &args.exclude_patterns,
            max_files,
        )?;

        if code_files.is_empty() {
            return Err("No code files found to analyze".to_string());
        }

        // Send progress event
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: format!("Found {} code files, sending to LLM for relevance analysis...", code_files.len()),
        }).map_err(|e| format!("Failed to send progress event: {}", e))?;

        // Step 2: Use LLM to determine relevant files
        let (relevant_files, llm_reasoning) = self.analyze_relevance_with_llm(
            &args.user_query,
            &code_files,
        ).await?;

        let execution_time_ms = start.elapsed().as_millis() as u64;

        let result = LargeContextFetchResult {
            relevant_files,
            llm_reasoning,
            total_files_analyzed: code_files.len() as u32,
            total_files_returned: 0, // Will be set below
            execution_time_ms,
        };

        // Update the count of returned files
        let mut final_result = result;
        final_result.total_files_returned = final_result.relevant_files.len() as u32;

        let result_json = serde_json::to_value(&final_result)
            .map_err(|e| format!("Failed to serialize result: {}", e))?;

        // Send result event
        self.event_sender.send(AppEvent::ToolResult {
            id: id.clone(),
            payload: self.truncate_result(result_json.clone()),
        }).map_err(|e| format!("Failed to send result event: {}", e))?;

        Ok(result_json)
    }

    fn gather_code_files(
        &self,
        base_path: &str,
        include_extensions: &Option<Vec<String>>,
        exclude_patterns: &Option<Vec<String>>,
        max_files: u32,
    ) -> Result<Vec<CodeFile>, String> {
        let path = Path::new(base_path);
        if !path.exists() {
            return Err(format!("Path does not exist: {}", base_path));
        }

        let default_extensions = vec![
            "rs".to_string(), "py".to_string(), "js".to_string(), "ts".to_string(),
            "tsx".to_string(), "jsx".to_string(), "go".to_string(), "java".to_string(),
            "cpp".to_string(), "c".to_string(), "h".to_string(), "hpp".to_string(),
            "cs".to_string(), "php".to_string(), "rb".to_string(), "swift".to_string(),
            "kt".to_string(), "scala".to_string(), "clj".to_string(), "hs".to_string(),
            "ml".to_string(), "elm".to_string(), "dart".to_string(), "vue".to_string(),
            "svelte".to_string(), "md".to_string(), "toml".to_string(), "yaml".to_string(),
            "yml".to_string(), "json".to_string(), "xml".to_string(),
        ];

        let extensions = include_extensions.as_ref().unwrap_or(&default_extensions);

        let default_exclude_patterns = vec![
            "target".to_string(),
            "node_modules".to_string(),
            ".git".to_string(),
            "dist".to_string(),
            "build".to_string(),
            "coverage".to_string(),
            ".cache".to_string(),
            "vendor".to_string(),
            "__pycache__".to_string(),
            ".pytest_cache".to_string(),
            "*.lock".to_string(),
        ];

        let exclude_patterns = exclude_patterns.as_ref().unwrap_or(&default_exclude_patterns);

        let mut code_files = Vec::new();
        let mut count = 0;

        for entry in WalkDir::new(path).max_depth(10) {
            if count >= max_files {
                break;
            }

            let entry = entry.map_err(|e| format!("Error walking directory: {}", e))?;
            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Check if path should be excluded
            let path_str = path.to_string_lossy();
            if exclude_patterns.iter().any(|pattern| {
                if pattern.contains('*') {
                    // Simple glob matching for patterns like "*.lock"
                    if pattern.starts_with("*.") {
                        let ext = &pattern[2..];
                        path_str.ends_with(ext)
                    } else {
                        path_str.contains(pattern.trim_start_matches('*'))
                    }
                } else {
                    path_str.contains(pattern)
                }
            }) {
                continue;
            }

            // Check file extension
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if !extensions.iter().any(|e| e.to_lowercase() == ext_str) {
                    continue;
                }
            } else {
                continue;
            }

            // Read file contents
            match std::fs::read_to_string(path) {
                Ok(contents) => {
                    let file_size = contents.len() as u64;
                    
                    // Skip very large files to prevent token overflow
                    if file_size > 100_000 {
                        continue;
                    }

                    // Detect language from extension
                    let language = path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.to_lowercase());

                    code_files.push(CodeFile {
                        path: path.to_string_lossy().to_string(),
                        contents,
                        language,
                        size_bytes: file_size,
                        truncated: false,
                    });

                    count += 1;
                }
                Err(_) => {
                    // Skip files that can't be read (binary files, permission issues, etc.)
                    continue;
                }
            }
        }

        Ok(code_files)
    }

    async fn analyze_relevance_with_llm(
        &self,
        user_query: &str,
        code_files: &[CodeFile],
    ) -> Result<(Vec<CodeFile>, String), String> {
        // Get API configuration from environment (same as MultiModelAgent)
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .or_else(|_| std::env::var("VERCEL_AI_GATEWAY_API_KEY"))
            .map_err(|_| "No API key found. Set OPENROUTER_API_KEY or VERCEL_AI_GATEWAY_API_KEY environment variable".to_string())?;

        let model = std::env::var("GROK_MODEL")
            .or_else(|_| std::env::var("VERCEL_AI_GATEWAY_MODEL"))
            .unwrap_or_else(|_| "anthropic/claude-3-haiku".to_string());

        // Allow tests or users to override base URL; otherwise choose by available key
        let base_url = std::env::var("GROK_LLM_BASE_URL").ok().unwrap_or_else(|| {
            if std::env::var("VERCEL_AI_GATEWAY_API_KEY").is_ok() {
                "https://ai-gateway.vercel.sh/v1/chat/completions".to_string()
            } else {
                "https://openrouter.ai/api/v1/chat/completions".to_string()
            }
        });

        // Create a summary of each file for the LLM
        let file_summaries: Vec<_> = code_files.iter().enumerate().map(|(i, file)| {
            let truncated_content = if file.contents.len() > 2000 {
                format!("{}...[truncated]", &file.contents[..2000])
            } else {
                file.contents.clone()
            };

            json!({
                "index": i,
                "path": file.path,
                "language": file.language,
                "size_bytes": file.size_bytes,
                "content": truncated_content
            })
        }).collect();

        let system_prompt = r#"You are a code analysis assistant. Your job is to analyze a list of code files and determine which ones are most relevant to a user's query.

For each file, consider:
- Does the file contain code that directly relates to the user's query?
- Does the file contain functions, classes, or concepts mentioned in the query?
- Is this file likely to contain the implementation or logic the user is asking about?
- Would this file help understand the broader context around the user's question?

You should be selective but not overly restrictive. Include files that are directly relevant as well as important context files that would help understand the relevant code.

Respond with:
1. A brief reasoning explaining your analysis
2. A JSON array of file indices (numbers) that are relevant

Format your response exactly like this:
REASONING: [Your reasoning here]
RELEVANT_FILES: [1, 5, 12, 23]"#;

        let user_message = format!(
            "User Query: {}\n\nCode Files to Analyze:\n{}",
            user_query,
            serde_json::to_string_pretty(&file_summaries)
                .map_err(|e| format!("Failed to serialize file summaries: {}", e))?
        );

        let body = json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_message}
            ],
            "temperature": 0.1,
            "max_tokens": 2000
        });

        // Make the HTTP request
        let client = reqwest::Client::new();
        let response = client
            .post(&base_url)
            .bearer_auth(&api_key)
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to make LLM request: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!("LLM API request failed with status {}: {}", status, error_text));
        }

        let response_json: Value = response.json().await
            .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

        // Extract the LLM's response
        let content = response_json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .ok_or_else(|| "Invalid LLM response format".to_string())?;

        // Parse the LLM's response
        let (reasoning, relevant_indices) = self.parse_llm_response(content)?;

        // Filter the files based on LLM's selection
        let relevant_files: Vec<CodeFile> = relevant_indices
            .into_iter()
            .filter_map(|i| code_files.get(i).cloned())
            .collect();

        Ok((relevant_files, reasoning))
    }

    fn parse_llm_response(&self, content: &str) -> Result<(String, Vec<usize>), String> {
        // Look for REASONING: and RELEVANT_FILES: sections
        let reasoning = if let Some(reasoning_start) = content.find("REASONING:") {
            let reasoning_text = &content[reasoning_start + 10..];
            if let Some(reasoning_end) = reasoning_text.find("RELEVANT_FILES:") {
                reasoning_text[..reasoning_end].trim().to_string()
            } else {
                reasoning_text.trim().to_string()
            }
        } else {
            // Fallback: use the entire response as reasoning
            content.trim().to_string()
        };

        let relevant_indices = if let Some(files_start) = content.find("RELEVANT_FILES:") {
            let files_text = &content[files_start + 15..];
            
            // Try to parse as JSON array
            if let Some(json_start) = files_text.find('[') {
                if let Some(json_end) = files_text.find(']') {
                    let json_str = &files_text[json_start..=json_end];
                    match serde_json::from_str::<Vec<usize>>(json_str) {
                        Ok(indices) => indices,
                        Err(_) => {
                            // Fallback: try to extract numbers from the text
                            self.extract_numbers_from_text(files_text)
                        }
                    }
                } else {
                    self.extract_numbers_from_text(files_text)
                }
            } else {
                self.extract_numbers_from_text(files_text)
            }
        } else {
            // If no RELEVANT_FILES section found, try to extract numbers from the entire response
            self.extract_numbers_from_text(content)
        };

        Ok((reasoning, relevant_indices))
    }

    pub fn extract_numbers_from_text(&self, text: &str) -> Vec<usize> {
        // Simple number extraction without regex to avoid dependency issues
        text.split_whitespace()
            .filter_map(|word| {
                // Try to parse numbers, handling common separators
                word.trim_matches(|c: char| !c.is_ascii_digit())
                    .parse::<usize>()
                    .ok()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::EventBus;
    use serde_json::json;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    fn make_executor(max_output_size: usize) -> LlmExecutor {
        let bus = EventBus::new();
        let sender = bus.sender();
        LlmExecutor::new(sender, max_output_size)
    }

    #[test]
    fn test_extract_numbers_from_text() {
        let exec = make_executor(10_000);
        let text = "Files: [1, 2, 10]; also #42 and (003).";
        let nums = exec.extract_numbers_from_text(text);
        assert_eq!(nums, vec![1, 2, 10, 42, 3]);
    }

    #[test]
    fn test_parse_llm_response_json_indices() {
        let exec = make_executor(10_000);
        let content = "REASONING: Picked files by direct relevance.\nRELEVANT_FILES: [0, 2, 5]";
        let (reason, indices) = exec.parse_llm_response(content).unwrap();
        assert!(reason.contains("Picked files"));
        assert_eq!(indices, vec![0, 2, 5]);
    }

    #[test]
    fn test_parse_llm_response_fallback_numbers() {
        let exec = make_executor(10_000);
        // Missing brackets should trigger fallback number extraction
        let content = "REASONING: ok\nRELEVANT_FILES: 0, 7; and 12.";
        let (_reason, indices) = exec.parse_llm_response(content).unwrap();
        assert_eq!(indices, vec![0, 7, 12]);
    }

    #[test]
    fn test_truncate_result_exceeds_limit() {
        let exec = make_executor(100);
        let big = json!({ "data": "x".repeat(500) });
        let out = exec.truncate_result(big);
        assert!(out.get("truncated").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(out.get("original_size_bytes").is_some());
        assert_eq!(out.get("max_allowed_bytes").and_then(|v| v.as_u64()), Some(100));
    }

    #[test]
    fn test_truncate_result_within_limit() {
        let exec = make_executor(10_000);
        let small = json!({ "ok": true, "n": 1 });
        let out = exec.truncate_result(small.clone());
        assert_eq!(out, small);
    }

    #[test]
    fn test_gather_code_files_filters_and_limits() {
        let exec = make_executor(10_000);
        let dir = tempdir().unwrap();
        let base = dir.path();

        // Allowed extension (.rs)
        let file_rs = base.join("main.rs");
        fs::write(&file_rs, "fn main() {}\n").unwrap();

        // Disallowed extension (.txt)
        let file_txt = base.join("notes.txt");
        fs::write(&file_txt, "not code").unwrap();

        // Excluded directory (node_modules)
        let nm_dir = base.join("node_modules");
        fs::create_dir(&nm_dir).unwrap();
        let nm_file = nm_dir.join("lib.rs");
        fs::write(&nm_file, "pub fn hidden() {}\n").unwrap();

        // Large file over 100_000 bytes should be skipped
        let big_file = base.join("big.rs");
        let mut f = fs::File::create(&big_file).unwrap();
        let big_content = "a".repeat(120_000);
        f.write_all(big_content.as_bytes()).unwrap();
        f.flush().unwrap();

        let base_str = base.to_string_lossy().to_string();
        let files = exec
            .gather_code_files(&base_str, &None, &None, 10)
            .unwrap();

        // Only the .rs within base (not excluded) and not too big should appear
        let paths: Vec<String> = files.iter().map(|f| f.path.clone()).collect();
        assert!(paths.iter().any(|p| p.ends_with("main.rs")));
        assert!(!paths.iter().any(|p| p.ends_with("notes.txt")));
        assert!(!paths.iter().any(|p| p.ends_with("node_modules/lib.rs")));
        assert!(!paths.iter().any(|p| p.ends_with("big.rs")));
    }
}
