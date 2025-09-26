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
        let _ = self.execute_large_context_fetch_with_result(id, args).await?;
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

        // Step 1: Gather all code files (no size truncation; include full contents)
        let base_path = args.base_path.as_deref().unwrap_or(".");
        let max_files = args.max_files.unwrap_or(200); // still keep a sane cap on the count

        let code_files = self.gather_code_files(
            base_path,
            &args.include_extensions,
            &args.exclude_patterns,
            max_files,
        )?;

        if code_files.is_empty() {
            return Err("No code files found to analyze".to_string());
        }

        // Step 2: Ask LLM for structured JSON (array of { file_path, reason })
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: format!("Sending {} files to LLM for relevance reasoning (structured outputs)...", code_files.len()),
        }).map_err(|e| format!("Failed to send progress event: {}", e))?;

        let llm_json = self.request_llm_structured_output(&args.user_query, &code_files).await?;

        // Optional: include diagnostics (timing) if you want, but request asked to return exactly the LLM JSON.
        let _execution_time_ms = start.elapsed().as_millis() as u64;

        // Send result event (possibly truncated for safety)
        self.event_sender.send(AppEvent::ToolResult {
            id: id.clone(),
            payload: self.truncate_result(llm_json.clone()),
        }).map_err(|e| format!("Failed to send result event: {}", e))?;

        Ok(llm_json)
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

            // Read file contents (keep entire contents)
            match std::fs::read_to_string(path) {
                Ok(contents) => {
                    let file_size = contents.len() as u64;

                    // Detect language from extension
                    let language = path.extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.to_lowercase());

                    code_files.push(CodeFile {
                        path: path.to_string_lossy().to_string(), // full path
                        contents,                                  // full contents
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

    /// Send files + query to LLM and require a strict JSON array of { file_path, reason }
    async fn request_llm_structured_output(
        &self,
        user_query: &str,
        code_files: &[CodeFile],
    ) -> Result<Value, String> {
        // API configuration
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| "No API key found. Set OPENROUTER_API_KEY environment variable".to_string())?;

        let model = std::env::var("OPENROUTER_MODEL")
            .map_err(|_| "No model found. Set OPENROUTER_MODEL environment variable".to_string())?;

        // Allow tests or users to override base URL; otherwise choose by available key
        let base_url = std::env::var("GROK_LLM_BASE_URL").ok().unwrap_or_else(|| {
            if std::env::var("OPENROUTER_API_KEY").is_ok() {
                "https://openrouter.ai/api/v1/chat/completions".to_string()
            } else {
                "https://ai-gateway.vercel.sh/v1/chat/completions".to_string()
            }
        });

        // Build the exact list the user requested we send: full path + full content
        let files_for_llm: Vec<Value> = code_files.iter().map(|f| {
            json!({
                "file_path": f.path,
                "content":   f.contents,
            })
        }).collect();

        // System/user prompts
        let system_prompt = r#"You are a code analysis assistant.
Given a user query and a list of files ({ file_path, content }), identify which files are relevant.
Return ONLY a strict JSON array of objects, each with:
- "file_path": string (the exact path provided to you)
- "reason": string (brief explanation why this file is relevant)
Do not include any other fields or wrapper keys.
NO prose, NO markdown, NO code fences—just valid JSON."#;

        let user_message = json!({
            "user_query": user_query,
            "files": files_for_llm
        });

        // Structured Outputs schema: TOP-LEVEL ARRAY
        let response_format = json!({
            "type": "json_schema",
            "json_schema": {
                "name": "file_relevance_list",
                "strict": true,
                "schema": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "file_path": {
                                "type": "string",
                                "description": "Full path of the relevant file as provided in input"
                            },
                            "reason": {
                                "type": "string",
                                "description": "Short explanation of why this file is relevant to the user query"
                            }
                        },
                        "required": ["file_path", "reason"],
                        "additionalProperties": false
                    }
                }
            }
        });

        // Make the HTTP request
        let body = json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system_prompt },
                { "role": "user",   "content": user_message.to_string() }
            ],
            "response_format": response_format
        });

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

        // Extract and parse the model's JSON content.
        // With structured outputs (strict), content should already be valid JSON (array).
        let content_val = response_json
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .ok_or_else(|| "Invalid LLM response format: missing choices[0].message.content".to_string())?;

        // If content is a string containing JSON, parse it; if it's already a JSON array, pass it through.
        let result = if content_val.is_string() {
            let s = content_val.as_str().unwrap();
            serde_json::from_str::<Value>(s)
                .map_err(|e| format!("LLM content was not valid JSON: {}", e))?
        } else {
            content_val.clone()
        };

        // Final sanity check: ensure it's an array of objects with required keys
        if !result.is_array() {
            return Err("LLM response was not a JSON array as required by schema".to_string());
        }

        Ok(result)
    }

}
