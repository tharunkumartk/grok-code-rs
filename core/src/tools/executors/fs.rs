use crate::events::{AppEvent, EventSender};
use crate::tools::types::*;
use serde_json::Value;
use std::path::Path;
use std::time::Instant;
use walkdir::WalkDir;
use globset::{Glob, GlobSet, GlobSetBuilder};

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

        // Note: we used to determine search_paths here, but now handle globs directly in the loop below

        // Precompile glob patterns (match against full paths by default; filename-only patterns are prefixed with **/)
        let compiled_globs: Option<GlobSet> = if let Some(globs) = &args.globs {
            if globs.is_empty() {
                None
            } else {
                let mut builder = GlobSetBuilder::new();
                for g in globs {
                    // "**/*" means match everything
                    if g == "**/*" { 
                        // Add a catch-all to ensure matches
                        builder.add(Glob::new("**/*").map_err(|e| format!("Invalid glob pattern {}: {}", g, e))?);
                        continue;
                    }
                    let pattern = if g.contains('/') { g.clone() } else { format!("**/{}", g) };
                    let glob = Glob::new(&pattern)
                        .map_err(|e| format!("Invalid glob pattern {}: {}", g, e))?;
                    builder.add(glob);
                }
                Some(builder.build().map_err(|e| format!("Failed to build globset: {}", e))?)
            }
        } else { None };

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
            if let Some(ref gs) = compiled_globs {
                if !gs.is_match(path) {
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
        let lines: Vec<&str> = diff.lines().collect();
        if lines.len() < 3 {
            return Err("Invalid patch format".to_string());
        }

        // Parse header lines to get file paths
        let mut old_file = None;
        let mut new_file = None;
        
        for line in &lines {
            if line.starts_with("--- ") {
                old_file = Some(line[4..].trim());
            } else if line.starts_with("+++ ") {
                new_file = Some(line[4..].trim());
            }
        }

        let old_path = old_file.ok_or("Could not find old file path in patch")?;
        let new_path = new_file.ok_or("Could not find new file path in patch")?;
        
        // Handle special cases
        let is_new_file = old_path == "/dev/null";
        let is_delete_file = new_path == "/dev/null";
        
        let file_path = if is_new_file {
            new_path
        } else if is_delete_file {
            old_path
        } else {
            new_path
        };
        
        if dry_run {
            if is_new_file {
                return Ok(format!("Dry run: would create new file {}", file_path));
            } else if is_delete_file {
                return Ok(format!("Dry run: would delete file {}", file_path));
            } else {
                return Ok(format!("Dry run: would modify {}", file_path));
            }
        }

        // Handle file creation
        if is_new_file {
            return self.apply_new_file_patch(new_path, &lines).await;
        }
        
        // Handle file deletion
        if is_delete_file {
            return self.apply_delete_file_patch(old_path).await;
        }
        
        // Handle file modification
        self.apply_modify_file_patch(file_path, &lines).await
    }
    
    async fn apply_new_file_patch(&self, file_path: &str, lines: &[&str]) -> Result<String, String> {
        let mut content = String::new();
        
        // Find the hunk start and extract added lines
        let mut in_hunk = false;
        for line in lines {
            if line.starts_with("@@") {
                in_hunk = true;
                continue;
            }
            
            if in_hunk {
                if line.starts_with('+') && !line.starts_with("+++") {
                    content.push_str(&line[1..]);
                    content.push('\n');
                }
            }
        }
        
        // Remove trailing newline if present to match expected behavior
        if content.ends_with('\n') {
            content.pop();
        }
        
        // Create parent directories if needed
        let path = std::path::Path::new(file_path);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await
                .map_err(|e| format!("Failed to create parent directories: {}", e))?;
        }
        
        // Write the new file
        tokio::fs::write(file_path, content).await
            .map_err(|e| format!("Failed to create new file {}: {}", file_path, e))?;
        
        Ok(format!("Created new file: {}", file_path))
    }
    
    async fn apply_delete_file_patch(&self, file_path: &str) -> Result<String, String> {
        if !std::path::Path::new(file_path).exists() {
            return Err(format!("File to delete does not exist: {}", file_path));
        }
        
        tokio::fs::remove_file(file_path).await
            .map_err(|e| format!("Failed to delete file {}: {}", file_path, e))?;
        
        Ok(format!("Deleted file: {}", file_path))
    }
    
    async fn apply_modify_file_patch(&self, file_path: &str, lines: &[&str]) -> Result<String, String> {
        // Read the original file
        let original_content = tokio::fs::read_to_string(file_path).await
            .map_err(|e| format!("Failed to read file {}: {}", file_path, e))?;
        
        let original_lines: Vec<&str> = original_content.lines().collect();
        let mut result_lines = Vec::new();
        let mut original_index = 0;
        
        // Parse hunks and apply changes
        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            
            if line.starts_with("@@") {
                // Parse hunk header to get line numbers
                let hunk_parts: Vec<&str> = line.split_whitespace().collect();
                if hunk_parts.len() >= 3 {
                    // Extract old line start from "-start,count" format
                    let old_range = hunk_parts[1];
                    if let Some(old_start_str) = old_range.strip_prefix('-') {
                        if let Some(comma_pos) = old_start_str.find(',') {
                            if let Ok(old_start) = old_start_str[..comma_pos].parse::<usize>() {
                                // Copy lines up to the hunk start (1-based to 0-based)
                                let hunk_start = old_start.saturating_sub(1);
                                while original_index < hunk_start && original_index < original_lines.len() {
                                    result_lines.push(original_lines[original_index].to_string());
                                    original_index += 1;
                                }
                            }
                        } else if let Ok(old_start) = old_start_str.parse::<usize>() {
                            // Handle single line format (no comma)
                            let hunk_start = old_start.saturating_sub(1);
                            while original_index < hunk_start && original_index < original_lines.len() {
                                result_lines.push(original_lines[original_index].to_string());
                                original_index += 1;
                            }
                        }
                    }
                }
                
                // Process hunk content
                i += 1;
                while i < lines.len() && !lines[i].starts_with("@@") {
                    let hunk_line = lines[i];
                    
                    if hunk_line.starts_with(' ') {
                        // Context line - keep as is
                        if original_index < original_lines.len() {
                            result_lines.push(original_lines[original_index].to_string());
                            original_index += 1;
                        }
                    } else if hunk_line.starts_with('-') && !hunk_line.starts_with("---") {
                        // Deleted line - skip in original
                        if original_index < original_lines.len() {
                            original_index += 1;
                        }
                    } else if hunk_line.starts_with('+') && !hunk_line.starts_with("+++") {
                        // Added line - add to result
                        result_lines.push(hunk_line[1..].to_string());
                    }
                    
                    i += 1;
                }
                continue;
            }
            
            i += 1;
        }
        
        // Add remaining original lines
        while original_index < original_lines.len() {
            result_lines.push(original_lines[original_index].to_string());
            original_index += 1;
        }
        
        let new_content = result_lines.join("\n");
        
        // Write the modified file
        tokio::fs::write(file_path, new_content).await
            .map_err(|e| format!("Failed to write modified file {}: {}", file_path, e))?;
        
        let additions = lines.iter()
            .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
            .count();
        let deletions = lines.iter()
            .filter(|line| line.starts_with('-') && !line.starts_with("---"))
            .count();
        
        Ok(format!("Modified file {}: {} insertions(+), {} deletions(-)", 
                   file_path, additions, deletions))
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
                // Support glob patterns using globset when fuzzy is disabled
                let mut builder = GlobSetBuilder::new();
                // If the pattern has a directory separator, match against full path; else match filename by prefixing **/
                let pattern = if pattern_to_match.contains('/') { pattern_to_match.clone() } else { format!("**/{}", pattern_to_match) };
                if let Ok(glob) = Glob::new(&pattern) {
                    builder.add(glob);
                    if let Ok(gs) = builder.build() {
                        if gs.is_match(path) {
                            let is_exact = name_to_match == pattern_to_match;
                            (true, if is_exact { "exact".to_string() } else { "partial".to_string() }, Some(if is_exact { 1.0 } else { 0.9 }))
                        } else if name_to_match.contains(&pattern_to_match) {
                            (true, "partial".to_string(), Some(0.9))
                        } else {
                            (false, "".to_string(), None)
                        }
                    } else {
                        // Fallback to substring on build error
                        if name_to_match.contains(&pattern_to_match) {
                            (true, "partial".to_string(), Some(0.9))
                        } else {
                            (false, "".to_string(), None)
                        }
                    }
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

// simple_glob_match has been replaced by globset-based matching in callers.

// Helper functions for fs.find (already defined above, keeping only detect_language)

fn detect_language(extension: &str) -> Option<String> {
    match extension {
        "rs" => Some("rust".to_string()),
        "py" => Some("python".to_string()),
        "js" => Some("javascript".to_string()),
        "ts" => Some("typescript".to_string()),
        "tsx" => Some("typescript".to_string()),
        "jsx" => Some("javascript".to_string()),
        "go" => Some("go".to_string()),
        "java" => Some("java".to_string()),
        "c" => Some("c".to_string()),
        "cpp" | "cxx" | "cc" => Some("cpp".to_string()),
        "h" | "hpp" | "hxx" => Some("c".to_string()),
        "cs" => Some("csharp".to_string()),
        "php" => Some("php".to_string()),
        "rb" => Some("ruby".to_string()),
        "swift" => Some("swift".to_string()),
        "kt" => Some("kotlin".to_string()),
        "scala" => Some("scala".to_string()),
        "r" => Some("r".to_string()),
        "m" => Some("objective-c".to_string()),
        "sh" | "bash" | "zsh" | "fish" => Some("bash".to_string()),
        "sql" => Some("sql".to_string()),
        "html" => Some("html".to_string()),
        "css" => Some("css".to_string()),
        "scss" | "sass" => Some("scss".to_string()),
        "less" => Some("less".to_string()),
        "vue" => Some("vue".to_string()),
        "svelte" => Some("svelte".to_string()),
        "elm" => Some("elm".to_string()),
        "clj" | "cljs" => Some("clojure".to_string()),
        "hs" => Some("haskell".to_string()),
        "ml" => Some("ocaml".to_string()),
        "fs" => Some("fsharp".to_string()),
        "pl" => Some("perl".to_string()),
        "lua" => Some("lua".to_string()),
        "dart" => Some("dart".to_string()),
        "julia" => Some("julia".to_string()),
        "nim" => Some("nim".to_string()),
        "zig" => Some("zig".to_string()),
        _ => None,
    }
}
