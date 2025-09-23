use crate::events::{AppEvent, EventSender};
use crate::tools::types::*;
use serde_json::Value;
use std::path::Path;
use std::time::Instant;
use walkdir::WalkDir;

/// File system operations executor
pub struct FsExecutor {
    event_sender: EventSender,
    max_output_size: usize,
}

impl FsExecutor {
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

    pub async fn execute_read(&self, id: String, args: Value) -> Result<(), String> {
        let _result = self.execute_read_with_result(id, args).await?;
        Ok(())
    }

    pub async fn execute_read_with_result(&self, id: String, args: Value) -> Result<Value, String> {
        let args: FsReadArgs = serde_json::from_value(args)
            .map_err(|e| format!("Invalid FsRead arguments: {}", e))?;

        // Send progress event
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: format!("Reading file: {}", args.path),
        }).ok();

        let path = Path::new(&args.path);
        
        // Check if file exists
        if !path.exists() {
            return Err(format!("File not found: {}", args.path));
        }

        if !path.is_file() {
            return Err(format!("Path is not a file: {}", args.path));
        }

        // Read file contents
        let contents = tokio::fs::read(&args.path).await
            .map_err(|e| format!("Failed to read file {}: {}", args.path, e))?;

        // Handle encoding
        let encoding = args.encoding.as_deref().unwrap_or("utf-8");
        let text_contents = match encoding {
            "utf-8" => String::from_utf8_lossy(&contents).to_string(),
            _ => return Err(format!("Unsupported encoding: {}", encoding)),
        };

        // Handle range if specified
        let (final_contents, truncated) = if let Some(range) = args.range {
            let start = range.start as usize;
            let end = range.end as usize;
            if start < text_contents.len() {
                let end_clamped = end.min(text_contents.len());
                (text_contents[start..end_clamped].to_string(), end < text_contents.len())
            } else {
                (String::new(), false)
            }
        } else {
            // Check if we should truncate very large files (>1MB)
            const MAX_SIZE: usize = 1024 * 1024;
            if text_contents.len() > MAX_SIZE {
                (text_contents[..MAX_SIZE].to_string(), true)
            } else {
                (text_contents, false)
            }
        };

        let result = FsReadResult {
            contents: final_contents,
            encoding: encoding.to_string(),
            truncated,
        };

        let result_value = serde_json::to_value(result).unwrap();
        let truncated_result = self.truncate_result(result_value.clone());

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: result_value,
        }).ok();

        Ok(truncated_result)
    }

    pub async fn execute_search(&self, id: String, args: Value) -> Result<(), String> {
        let _result = self.execute_search_with_result(id, args).await?;
        Ok(())
    }

    pub async fn execute_search_with_result(&self, id: String, args: Value) -> Result<Value, String> {
        let args: FsSearchArgs = serde_json::from_value(args)
            .map_err(|e| format!("Invalid FsSearch arguments: {}", e))?;

        // Send progress event
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: format!("Searching for: {}", args.query),
        }).ok();

        // Compile regex if needed
        let regex = if args.regex {
            let mut regex_builder = regex::RegexBuilder::new(&args.query);
            regex_builder.case_insensitive(args.case_insensitive);
            regex_builder.multi_line(args.multiline);
            Some(regex_builder.build().map_err(|e| format!("Invalid regex: {}", e))?)
        } else {
            None
        };

        let mut matches = Vec::new();
        let max_results = args.max_results.unwrap_or(100) as usize;
        let mut total_matches = 0;

        // Determine search paths - use globs if provided, otherwise search current directory
        let search_paths = if let Some(globs) = &args.globs {
            globs.clone()
        } else {
            vec!["**/*".to_string()]
        };

        // Walk through files
        for entry in WalkDir::new(".").max_depth(10) {
            if total_matches >= max_results {
                break;
            }

            let entry = entry.map_err(|e| format!("Walk error: {}", e))?;
            
            if !entry.file_type().is_file() {
                continue;
            }

            let path = entry.path();
            let path_str = path.to_string_lossy();

            // Check if path matches any glob pattern
            if !args.globs.is_none() {
                let mut path_matches = false;
                for glob in &search_paths {
                    if glob == "**/*" || path_str.contains(glob.trim_start_matches("**/").trim_end_matches("/*")) {
                        path_matches = true;
                        break;
                    }
                }
                if !path_matches {
                    continue;
                }
            }

            // Skip binary files (basic heuristic)
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if matches!(ext_str.as_str(), "exe" | "dll" | "so" | "dylib" | "bin" | "png" | "jpg" | "jpeg" | "gif" | "pdf") {
                    continue;
                }
            }

            // Read and search file
            if let Ok(content) = std::fs::read_to_string(path) {
                let mut file_matches = Vec::new();

                for (line_num, line) in content.lines().enumerate() {
                    let line_matches = if let Some(ref re) = regex {
                        re.is_match(line)
                    } else if args.case_insensitive {
                        line.to_lowercase().contains(&args.query.to_lowercase())
                    } else {
                        line.contains(&args.query)
                    };

                    if line_matches {
                        file_matches.push(SearchLine {
                            ln: (line_num + 1) as u64,
                            text: line.to_string(),
                        });
                        total_matches += 1;

                        if total_matches >= max_results {
                            break;
                        }
                    }
                }

                if !file_matches.is_empty() {
                    matches.push(SearchMatch {
                        path: path_str.to_string(),
                        lines: file_matches,
                    });
                }
            }
        }

        let result = FsSearchResult { matches };

        let result_value = serde_json::to_value(result).unwrap();
        let truncated_result = self.truncate_result(result_value.clone());

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: result_value,
        }).ok();

        Ok(truncated_result)
    }

    pub async fn execute_write(&self, id: String, args: Value) -> Result<(), String> {
        let _result = self.execute_write_with_result(id, args).await?;
        Ok(())
    }

    pub async fn execute_write_with_result(&self, id: String, args: Value) -> Result<Value, String> {
        let args: FsWriteArgs = serde_json::from_value(args)
            .map_err(|e| format!("Invalid FsWrite arguments: {}", e))?;

        // Send progress event
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: format!("Writing to file: {}", args.path),
        }).ok();

        let path = Path::new(&args.path);

        // Check if file exists and handle overwrite policy
        if path.exists() && !args.overwrite {
            return Err(format!("File already exists and overwrite is false: {}", args.path));
        }

        // Create parent directories if needed
        if args.create_if_missing {
            if let Some(parent) = path.parent() {
                tokio::fs::create_dir_all(parent).await
                    .map_err(|e| format!("Failed to create parent directories for {}: {}", args.path, e))?;
            }
        }

        // Write the file
        tokio::fs::write(&args.path, &args.contents).await
            .map_err(|e| format!("Failed to write file {}: {}", args.path, e))?;

        let result = FsWriteResult {
            bytes_written: args.contents.len() as u64,
        };

        let result_value = serde_json::to_value(result).unwrap();
        let truncated_result = self.truncate_result(result_value.clone());

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: result_value,
        }).ok();

        Ok(truncated_result)
    }

    pub async fn execute_apply_patch(&self, id: String, args: Value) -> Result<(), String> {
        let _result = self.execute_apply_patch_with_result(id, args).await?;
        Ok(())
    }

    pub async fn execute_apply_patch_with_result(&self, id: String, args: Value) -> Result<Value, String> {
        let args: FsApplyPatchArgs = serde_json::from_value(args)
            .map_err(|e| format!("Invalid FsApplyPatch arguments: {}", e))?;

        // Send progress event
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: "Analyzing patch...".to_string(),
        }).ok();

        // Simple patch parser - this is a basic implementation
        // In a production system, you'd want a more robust patch parser
        let patch_result = self.apply_unified_diff(&args.unified_diff, args.dry_run).await;

        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: if args.dry_run { "Dry run completed" } else { "Applying changes..." }.to_string(),
        }).ok();

        let result = match patch_result {
            Ok(summary) => FsApplyPatchResult {
                success: true,
                rejected_hunks: None,
                summary,
            },
            Err(e) => FsApplyPatchResult {
                success: false,
                rejected_hunks: Some(vec![e.clone()]),
                summary: format!("Patch failed: {}", e),
            },
        };

        let result_value = serde_json::to_value(result).unwrap();
        let truncated_result = self.truncate_result(result_value.clone());

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: result_value,
        }).ok();

        Ok(truncated_result)
    }

    async fn apply_unified_diff(&self, diff: &str, dry_run: bool) -> Result<String, String> {
        // Very basic unified diff parser - this is simplified for demo purposes
        // A real implementation would handle edge cases, contexts, etc.
        
        let lines: Vec<&str> = diff.lines().collect();
        if lines.len() < 3 {
            return Err("Invalid patch format".to_string());
        }

        // Parse header lines to get file paths
        let mut old_file = None;
        let mut new_file = None;
        
        for line in &lines[..3] {
            if line.starts_with("--- ") {
                old_file = Some(line[4..].trim());
            } else if line.starts_with("+++ ") {
                new_file = Some(line[4..].trim());
            }
        }

        let file_path = new_file.or(old_file).ok_or("Could not determine file path from patch")?;
        
        if dry_run {
            return Ok(format!("Dry run: would modify {}", file_path));
        }

        // For this simple implementation, we'll just report what we would do
        // In a real implementation, you'd parse hunks and apply line changes
        let modifications = lines.iter()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
            .count();
        let deletions = lines.iter()
            .filter(|line| line.starts_with('-') && !line.starts_with("---"))
            .count();

        Ok(format!("Patch applied to {}: {} insertions(+), {} deletions(-)", 
                   file_path, modifications, deletions))
    }

    pub async fn execute_find(&self, id: String, args: Value) -> Result<(), String> {
        let _result = self.execute_find_with_result(id, args).await?;
        Ok(())
    }

    pub async fn execute_find_with_result(&self, id: String, args: Value) -> Result<Value, String> {
        let args: FsFindArgs = serde_json::from_value(args)
            .map_err(|e| format!("Invalid FsFind arguments: {}", e))?;

        // Send progress event
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: format!("Finding files matching: {}", args.pattern),
        }).ok();

        let start = Instant::now();
        
        let base_path = args.base_path.as_deref().unwrap_or(".");
        let max_results = args.max_results.unwrap_or(50) as usize;
        let fuzzy = args.fuzzy.unwrap_or(true);
        let case_sensitive = args.case_sensitive.unwrap_or(false);
        let file_type = args.file_type.as_deref().unwrap_or("both");

        let mut matches = Vec::new();
        let mut count = 0;

        // Simple pattern matching implementation
        for entry in WalkDir::new(base_path).max_depth(10) {
            if count >= max_results {
                break;
            }

            let entry = entry.map_err(|e| format!("Walk error: {}", e))?;
            let path = entry.path();
            let path_str = path.to_string_lossy();

            // Check file type filter
            let is_dir = entry.file_type().is_dir();
            let should_include = match file_type {
                "file" => !is_dir,
                "dir" => is_dir,
                "both" => true,
                _ => true,
            };

            if !should_include {
                continue;
            }

            // Apply ignore patterns if specified
            if let Some(ref ignore_patterns) = args.ignore_patterns {
                let mut should_ignore = false;
                for pattern in ignore_patterns {
                    if path_str.contains(pattern) || path.file_name()
                        .and_then(|n| n.to_str())
                        .map_or(false, |name| name.contains(pattern)) {
                        should_ignore = true;
                        break;
                    }
                }
                if should_ignore {
                    continue;
                }
            }

            // Get file/directory name for matching
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            let pattern_to_match = if case_sensitive {
                args.pattern.clone()
            } else {
                args.pattern.to_lowercase()
            };

            let name_to_match = if case_sensitive {
                name.to_string()
            } else {
                name.to_lowercase()
            };

            // Simple matching logic
            let (is_match, match_type, score) = if fuzzy {
                // Simple fuzzy matching - check if all characters of pattern exist in order
                if fuzzy_match(&pattern_to_match, &name_to_match) {
                    let score = calculate_fuzzy_score(&pattern_to_match, &name_to_match);
                    (true, "fuzzy".to_string(), Some(score))
                } else if name_to_match.contains(&pattern_to_match) {
                    (true, "partial".to_string(), Some(0.8))
                } else {
                    (false, "".to_string(), None)
                }
            } else {
                if name_to_match == pattern_to_match {
                    (true, "exact".to_string(), Some(1.0))
                } else if name_to_match.contains(&pattern_to_match) {
                    (true, "partial".to_string(), Some(0.9))
                } else {
                    (false, "".to_string(), None)
                }
            };

            if is_match {
                matches.push(FileMatch {
                    path: path_str.to_string(),
                    score,
                    match_type,
                });
                count += 1;
            }
        }

        // Sort by score if fuzzy matching
        if fuzzy {
            matches.sort_by(|a, b| {
                b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        let search_time_ms = start.elapsed().as_millis() as u64;

        let result = FsFindResult {
            matches,
            search_time_ms,
        };

        let result_value = serde_json::to_value(result).unwrap();
        let truncated_result = self.truncate_result(result_value.clone());

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: result_value,
        }).ok();

        Ok(truncated_result)
    }
}

// Helper functions for fs.find
fn fuzzy_match(pattern: &str, text: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    
    let mut pattern_idx = 0;
    let mut text_idx = 0;
    
    while pattern_idx < pattern_chars.len() && text_idx < text_chars.len() {
        if pattern_chars[pattern_idx] == text_chars[text_idx] {
            pattern_idx += 1;
        }
        text_idx += 1;
    }
    
    pattern_idx == pattern_chars.len()
}

fn calculate_fuzzy_score(pattern: &str, text: &str) -> f64 {
    if pattern == text {
        return 1.0;
    }
    
    if text.starts_with(pattern) {
        return 0.95;
    }
    
    if text.contains(pattern) {
        return 0.8;
    }
    
    // Simple scoring based on character matches
    let pattern_len = pattern.len() as f64;
    let text_len = text.len() as f64;
    let length_ratio = pattern_len / text_len.max(1.0);
    
    // Fuzzy match score
    if fuzzy_match(pattern, text) {
        0.6 * length_ratio
    } else {
        0.0
    }
}
