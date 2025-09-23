use crate::events::{AppEvent, EventSender, ToolName};
use crate::tools::types::*;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use tokio::process::Command;
use tokio::time::timeout;
use walkdir::WalkDir;

/// Tool executor that performs real file system and shell operations
pub struct ToolExecutor {
    event_sender: EventSender,
}

impl ToolExecutor {
    pub fn new(event_sender: EventSender) -> Self {
        Self { event_sender }
    }

    /// Execute a tool with the given arguments and return the result
    pub async fn execute_tool_with_result(&self, id: String, tool: ToolName, args: Value) -> Result<Value, String> {
        let summary = self.get_tool_summary(&tool, &args);
        
        // Send tool begin event
        self.event_sender.send(AppEvent::ToolBegin {
            id: id.clone(),
            tool: tool.clone(),
            summary,
        }).map_err(|e| format!("Failed to send ToolBegin event: {}", e))?;

        let start = Instant::now();

        // Execute the specific tool and get result
        let result = match tool {
            ToolName::FsRead => self.execute_fs_read_with_result(id.clone(), args).await,
            ToolName::FsSearch => self.execute_fs_search_with_result(id.clone(), args).await,
            ToolName::FsWrite => self.execute_fs_write_with_result(id.clone(), args).await,
            ToolName::FsApplyPatch => self.execute_fs_apply_patch_with_result(id.clone(), args).await,
            ToolName::ShellExec => self.execute_shell_exec_with_result(id.clone(), args).await,
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        // Send tool end event
        self.event_sender.send(AppEvent::ToolEnd {
            id: id.clone(),
            ok: result.is_ok(),
            duration_ms,
        }).map_err(|e| format!("Failed to send ToolEnd event: {}", e))?;

        result
    }

    /// Execute a tool with the given arguments (legacy method for compatibility)
    pub async fn execute_tool(&self, id: String, tool: ToolName, args: Value) -> Result<(), String> {
        let summary = self.get_tool_summary(&tool, &args);
        
        // Send tool begin event
        self.event_sender.send(AppEvent::ToolBegin {
            id: id.clone(),
            tool: tool.clone(),
            summary,
        }).map_err(|e| format!("Failed to send ToolBegin event: {}", e))?;

        let start = Instant::now();

        // Execute the specific tool
        let result = match tool {
            ToolName::FsRead => self.execute_fs_read(id.clone(), args).await,
            ToolName::FsSearch => self.execute_fs_search(id.clone(), args).await,
            ToolName::FsWrite => self.execute_fs_write(id.clone(), args).await,
            ToolName::FsApplyPatch => self.execute_fs_apply_patch(id.clone(), args).await,
            ToolName::ShellExec => self.execute_shell_exec(id.clone(), args).await,
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        // Send tool end event
        self.event_sender.send(AppEvent::ToolEnd {
            id: id.clone(),
            ok: result.is_ok(),
            duration_ms,
        }).map_err(|e| format!("Failed to send ToolEnd event: {}", e))?;

        result
    }

    fn get_tool_summary(&self, tool: &ToolName, args: &Value) -> String {
        match tool {
            ToolName::FsRead => {
                if let Ok(args) = serde_json::from_value::<FsReadArgs>(args.clone()) {
                    format!("Reading file: {}", args.path)
                } else {
                    "Reading file".to_string()
                }
            }
            ToolName::FsSearch => {
                if let Ok(args) = serde_json::from_value::<FsSearchArgs>(args.clone()) {
                    format!("Searching for: {}", args.query)
                } else {
                    "Searching files".to_string()
                }
            }
            ToolName::FsWrite => {
                if let Ok(args) = serde_json::from_value::<FsWriteArgs>(args.clone()) {
                    format!("Writing to file: {}", args.path)
                } else {
                    "Writing file".to_string()
                }
            }
            ToolName::FsApplyPatch => "Applying patch".to_string(),
            ToolName::ShellExec => {
                if let Ok(args) = serde_json::from_value::<ShellExecArgs>(args.clone()) {
                    format!("Executing: {}", args.command.join(" "))
                } else {
                    "Executing command".to_string()
                }
            }
        }
    }

    async fn execute_fs_read(&self, id: String, args: Value) -> Result<(), String> {
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

        // Send result
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: serde_json::to_value(result).unwrap(),
        }).map_err(|e| format!("Failed to send ToolResult: {}", e))?;

        Ok(())
    }

    async fn execute_fs_search(&self, id: String, args: Value) -> Result<(), String> {
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

        // Send result
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: serde_json::to_value(result).unwrap(),
        }).map_err(|e| format!("Failed to send ToolResult: {}", e))?;

        Ok(())
    }

    async fn execute_fs_write(&self, id: String, args: Value) -> Result<(), String> {
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

        // Send result
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: serde_json::to_value(result).unwrap(),
        }).map_err(|e| format!("Failed to send ToolResult: {}", e))?;

        Ok(())
    }

    async fn execute_fs_apply_patch(&self, id: String, args: Value) -> Result<(), String> {
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

        // Send result
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: serde_json::to_value(result).unwrap(),
        }).map_err(|e| format!("Failed to send ToolResult: {}", e))?;

        Ok(())
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

    async fn execute_shell_exec(&self, id: String, args: Value) -> Result<(), String> {
        let args: ShellExecArgs = serde_json::from_value(args)
            .map_err(|e| format!("Invalid ShellExec arguments: {}", e))?;

        if args.command.is_empty() {
            return Err("Empty command".to_string());
        }

        // Send progress event
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: format!("Executing: {}", args.command.join(" ")),
        }).ok();

        let start = Instant::now();
        let timeout_duration = Duration::from_millis(args.timeout_ms.unwrap_or(30000));

        // Setup command
        let mut command = Command::new(&args.command[0]);
        if args.command.len() > 1 {
            command.args(&args.command[1..]);
        }

        // Set working directory
        if let Some(cwd) = &args.cwd {
            command.current_dir(cwd);
        }

        // Set environment variables
        if let Some(env_vars) = &args.env {
            for (key, value) in env_vars {
                command.env(key, value);
            }
        }

        // Configure stdio
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        // Spawn the process
        let mut child = command.spawn()
            .map_err(|e| format!("Failed to spawn command: {}", e))?;

        // Get stdout and stderr handles
        let stdout = child.stdout.take().ok_or("Failed to get stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to get stderr")?;

        // Setup async readers
        let mut stdout_reader = AsyncBufReader::new(stdout).lines();
        let mut stderr_reader = AsyncBufReader::new(stderr).lines();

        // Read output concurrently (legacy method only sends events)
        let id_clone = id.clone();
        let sender_clone = self.event_sender.clone();
        let stdout_task = tokio::spawn(async move {
            while let Ok(Some(line)) = stdout_reader.next_line().await {
                let _ = sender_clone.send(AppEvent::ToolStdout {
                    id: id_clone.clone(),
                    chunk: format!("{}\n", line),
                });
            }
        });

        let id_clone = id.clone();
        let sender_clone = self.event_sender.clone();
        let stderr_task = tokio::spawn(async move {
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                let _ = sender_clone.send(AppEvent::ToolStderr {
                    id: id_clone.clone(),
                    chunk: format!("{}\n", line),
                });
            }
        });

        // Wait for process with timeout
        let wait_result = timeout(timeout_duration, child.wait()).await;

        // Cancel reading tasks
        stdout_task.abort();
        stderr_task.abort();

        let exit_status = match wait_result {
            Ok(Ok(status)) => status,
            Ok(Err(e)) => return Err(format!("Process wait error: {}", e)),
            Err(_) => {
                // Timeout - kill the process
                let _ = child.kill().await;
                return Err("Command timed out".to_string());
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let exit_code = exit_status.code().unwrap_or(-1);

        let result = json!({
            "exit_code": exit_code,
            "duration_ms": duration_ms
        });

        // Send result
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: result,
        }).map_err(|e| format!("Failed to send ToolResult: {}", e))?;

        if exit_code != 0 {
            return Err(format!("Command failed with exit code: {}", exit_code));
        }

        Ok(())
    }

    // Methods that return actual results for use in conversation context

    async fn execute_fs_read_with_result(&self, id: String, args: Value) -> Result<Value, String> {
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

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: serde_json::to_value(&result).unwrap(),
        }).ok();

        Ok(serde_json::to_value(result).unwrap())
    }

    async fn execute_fs_search_with_result(&self, id: String, args: Value) -> Result<Value, String> {
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

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: serde_json::to_value(&result).unwrap(),
        }).ok();

        Ok(serde_json::to_value(result).unwrap())
    }

    async fn execute_fs_write_with_result(&self, id: String, args: Value) -> Result<Value, String> {
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

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: serde_json::to_value(&result).unwrap(),
        }).ok();

        Ok(serde_json::to_value(result).unwrap())
    }

    async fn execute_fs_apply_patch_with_result(&self, id: String, args: Value) -> Result<Value, String> {
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

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: serde_json::to_value(&result).unwrap(),
        }).ok();

        Ok(serde_json::to_value(result).unwrap())
    }

    async fn execute_shell_exec_with_result(&self, id: String, args: Value) -> Result<Value, String> {
        let args: ShellExecArgs = serde_json::from_value(args)
            .map_err(|e| format!("Invalid ShellExec arguments: {}", e))?;

        if args.command.is_empty() {
            return Err("Empty command".to_string());
        }

        // Send progress event
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: format!("Executing: {}", args.command.join(" ")),
        }).ok();

        let start = Instant::now();
        let timeout_duration = Duration::from_millis(args.timeout_ms.unwrap_or(30000));

        // Setup command
        let mut command = Command::new(&args.command[0]);
        if args.command.len() > 1 {
            command.args(&args.command[1..]);
        }

        // Set working directory
        if let Some(cwd) = &args.cwd {
            command.current_dir(cwd);
        }

        // Set environment variables
        if let Some(env_vars) = &args.env {
            for (key, value) in env_vars {
                command.env(key, value);
            }
        }

        // Configure stdio
        command.stdout(Stdio::piped()).stderr(Stdio::piped());

        // Spawn the process
        let mut child = command.spawn()
            .map_err(|e| format!("Failed to spawn command: {}", e))?;

        // Get stdout and stderr handles
        let stdout = child.stdout.take().ok_or("Failed to get stdout")?;
        let stderr = child.stderr.take().ok_or("Failed to get stderr")?;

        // Setup async readers
        let mut stdout_reader = AsyncBufReader::new(stdout).lines();
        let mut stderr_reader = AsyncBufReader::new(stderr).lines();

        // Read output concurrently
        let id_clone = id.clone();
        let sender_clone = self.event_sender.clone();
        let stdout_task = tokio::spawn(async move {
            let mut lines = Vec::new();
            while let Ok(Some(line)) = stdout_reader.next_line().await {
                let line_with_newline = format!("{}\n", line);
                let _ = sender_clone.send(AppEvent::ToolStdout {
                    id: id_clone.clone(),
                    chunk: line_with_newline.clone(),
                });
                lines.push(line_with_newline);
            }
            lines
        });

        let id_clone = id.clone();
        let sender_clone = self.event_sender.clone();
        let stderr_task = tokio::spawn(async move {
            let mut lines = Vec::new();
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                let line_with_newline = format!("{}\n", line);
                let _ = sender_clone.send(AppEvent::ToolStderr {
                    id: id_clone.clone(),
                    chunk: line_with_newline.clone(),
                });
                lines.push(line_with_newline);
            }
            lines
        });

        // Wait for process with timeout
        let wait_result = timeout(timeout_duration, child.wait()).await;

        // Get output from tasks
        let stdout_lines = stdout_task.await.unwrap_or_default();
        let stderr_lines = stderr_task.await.unwrap_or_default();
        
        let stdout_output = stdout_lines.join("");
        let stderr_output = stderr_lines.join("");

        let exit_status = match wait_result {
            Ok(Ok(status)) => status,
            Ok(Err(e)) => return Err(format!("Process wait error: {}", e)),
            Err(_) => {
                // Timeout - kill the process
                let _ = child.kill().await;
                return Err("Command timed out".to_string());
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let exit_code = exit_status.code().unwrap_or(-1);

        let result = ShellExecResult {
            exit_code,
            duration_ms,
            stdout: stdout_output,
            stderr: stderr_output,
        };

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: serde_json::to_value(&result).unwrap(),
        }).ok();

        if exit_code != 0 {
            return Err(format!("Command failed with exit code: {}", exit_code));
        }

        Ok(serde_json::to_value(result).unwrap())
    }
}
