use crate::events::{AppEvent, EventSender, ToolName};
use crate::tools::types::*;
use crate::tools::executors::{FsExecutor, ShellExecutor, CodeExecutor};
use serde_json::Value;
use std::time::Instant;

/// Tool executor that performs real file system and shell operations
pub struct ToolExecutor {
    event_sender: EventSender,
    max_output_size: usize,
    fs_executor: FsExecutor,
    shell_executor: ShellExecutor,
    code_executor: CodeExecutor,
}

impl ToolExecutor {
    pub fn new(event_sender: EventSender) -> Self {
        let max_output_size = std::env::var("GROK_TOOL_MAX_OUTPUT_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(1024 * 1024); // 1MB default

        let fs_executor = FsExecutor::new(event_sender.clone(), max_output_size);
        let shell_executor = ShellExecutor::new(event_sender.clone(), max_output_size);
        let code_executor = CodeExecutor::new(event_sender.clone(), max_output_size);

        Self {
            event_sender,
            max_output_size,
            fs_executor,
            shell_executor,
            code_executor,
        }
    }

    pub fn with_max_output_size(mut self, max_output_size: usize) -> Self {
        self.max_output_size = max_output_size;
        self.fs_executor = FsExecutor::new(self.event_sender.clone(), max_output_size);
        self.shell_executor = ShellExecutor::new(self.event_sender.clone(), max_output_size);
        self.code_executor = CodeExecutor::new(self.event_sender.clone(), max_output_size);
        self
    }


    /// Execute a tool with the given arguments and return the result
    pub async fn execute_tool_with_result(&self, id: String, tool: ToolName, args: Value) -> Result<Value, String> {
        let summary = self.get_tool_summary(&tool, &args);
        
        // Send tool begin event
        self.event_sender.send(AppEvent::ToolBegin {
            id: id.clone(),
            tool: tool.clone(),
            summary,
            args: Some(args.clone()),
        }).map_err(|e| format!("Failed to send ToolBegin event: {}", e))?;

        let start = Instant::now();

        // Execute the specific tool and get result
        let result = match tool {
            ToolName::FsRead => self.fs_executor.execute_read_with_result(id.clone(), args).await,
            ToolName::FsSearch => self.fs_executor.execute_search_with_result(id.clone(), args).await,
            ToolName::FsWrite => self.fs_executor.execute_write_with_result(id.clone(), args).await,
            ToolName::FsApplyPatch => self.fs_executor.execute_apply_patch_with_result(id.clone(), args).await,
            ToolName::FsFind => self.fs_executor.execute_find_with_result(id.clone(), args).await,
            ToolName::ShellExec => self.shell_executor.execute_with_result(id.clone(), args).await,
            ToolName::CodeSymbols => self.code_executor.execute_symbols_with_result(id.clone(), args).await,
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
            args: Some(args.clone()),
        }).map_err(|e| format!("Failed to send ToolBegin event: {}", e))?;

        let start = Instant::now();

        // Execute the specific tool
        let result = match tool {
            ToolName::FsRead => self.fs_executor.execute_read(id.clone(), args).await,
            ToolName::FsSearch => self.fs_executor.execute_search(id.clone(), args).await,
            ToolName::FsWrite => self.fs_executor.execute_write(id.clone(), args).await,
            ToolName::FsApplyPatch => self.fs_executor.execute_apply_patch(id.clone(), args).await,
            ToolName::FsFind => self.fs_executor.execute_find(id.clone(), args).await,
            ToolName::ShellExec => self.shell_executor.execute(id.clone(), args).await,
            ToolName::CodeSymbols => self.code_executor.execute_symbols(id.clone(), args).await,
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
            ToolName::FsFind => {
                if let Ok(args) = serde_json::from_value::<FsFindArgs>(args.clone()) {
                    format!("Finding files: {}", args.pattern)
                } else {
                    "Finding files".to_string()
                }
            }
            ToolName::ShellExec => {
                if let Ok(args) = serde_json::from_value::<ShellExecArgs>(args.clone()) {
                    format!("Executing: {}", args.command.join(" "))
                } else {
                    "Executing command".to_string()
                }
            }
            ToolName::CodeSymbols => {
                if let Ok(args) = serde_json::from_value::<CodeSymbolsArgs>(args.clone()) {
                    format!("Analyzing symbols in: {}", args.path)
                } else {
                    "Analyzing code symbols".to_string()
                }
            }
        }
    }
}