use crate::events::{AppEvent, EventSender, ToolName};
use crate::tools::types::*;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Mock tool executor that simulates tool execution for UI testing
pub struct ToolExecutor {
    event_sender: EventSender,
}

impl ToolExecutor {
    pub fn new(event_sender: EventSender) -> Self {
        Self { event_sender }
    }

    /// Execute a tool with the given arguments
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

        // Simulate file reading delay
        sleep(Duration::from_millis(200)).await;

        // Mock result
        let result = FsReadResult {
            contents: format!("Mock contents of file: {}\n\nThis is simulated file content for testing the UI.", args.path),
            encoding: "utf-8".to_string(),
            truncated: false,
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

        // Simulate search delay
        sleep(Duration::from_millis(500)).await;

        // Mock search results
        let result = FsSearchResult {
            matches: vec![
                SearchMatch {
                    path: "src/main.rs".to_string(),
                    lines: vec![
                        SearchLine { ln: 10, text: format!("// Found match: {}", args.query) },
                        SearchLine { ln: 25, text: format!("let {} = mock_value();", args.query) },
                    ],
                },
                SearchMatch {
                    path: "src/lib.rs".to_string(),
                    lines: vec![
                        SearchLine { ln: 5, text: format!("pub fn {}() {{", args.query) },
                    ],
                },
            ],
        };

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

        // Simulate write delay
        sleep(Duration::from_millis(300)).await;

        // Mock result
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

        sleep(Duration::from_millis(200)).await;

        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: "Applying changes...".to_string(),
        }).ok();

        sleep(Duration::from_millis(400)).await;

        // Mock result
        let result = FsApplyPatchResult {
            success: !args.dry_run, // Succeed unless it's a dry run (for demo)
            rejected_hunks: if args.dry_run { 
                Some(vec!["Hunk #1 at line 10".to_string()]) 
            } else { 
                None 
            },
            summary: if args.dry_run {
                "Dry run completed. 1 hunk would be rejected.".to_string()
            } else {
                "Patch applied successfully. 3 files changed, 15 insertions(+), 8 deletions(-)".to_string()
            },
        };

        // Send result
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: serde_json::to_value(result).unwrap(),
        }).map_err(|e| format!("Failed to send ToolResult: {}", e))?;

        Ok(())
    }

    async fn execute_shell_exec(&self, id: String, args: Value) -> Result<(), String> {
        let args: ShellExecArgs = serde_json::from_value(args)
            .map_err(|e| format!("Invalid ShellExec arguments: {}", e))?;

        // Send progress event
        self.event_sender.send(AppEvent::ToolProgress {
            id: id.clone(),
            message: format!("Executing: {}", args.command.join(" ")),
        }).ok();

        // Simulate command output
        sleep(Duration::from_millis(100)).await;

        // Send some stdout
        self.event_sender.send(AppEvent::ToolStdout {
            id: id.clone(),
            chunk: format!("Starting execution of: {}\n", args.command.join(" ")),
        }).ok();

        sleep(Duration::from_millis(200)).await;

        self.event_sender.send(AppEvent::ToolStdout {
            id: id.clone(),
            chunk: "Processing...\n".to_string(),
        }).ok();

        sleep(Duration::from_millis(300)).await;

        // Occasionally send stderr for demonstration
        if args.command.contains(&"test".to_string()) {
            self.event_sender.send(AppEvent::ToolStderr {
                id: id.clone(),
                chunk: "Warning: This is a test command\n".to_string(),
            }).ok();
        }

        sleep(Duration::from_millis(200)).await;

        self.event_sender.send(AppEvent::ToolStdout {
            id: id.clone(),
            chunk: "Command completed successfully!\n".to_string(),
        }).ok();

        // Mock result
        let result = ShellExecResult {
            exit_code: 0,
            duration_ms: 800,
        };

        // Send result
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: serde_json::to_value(result).unwrap(),
        }).map_err(|e| format!("Failed to send ToolResult: {}", e))?;

        Ok(())
    }
}
