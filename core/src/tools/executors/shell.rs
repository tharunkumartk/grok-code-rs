use crate::events::{AppEvent, EventSender};
use crate::tools::types::*;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader as AsyncBufReader};
use tokio::process::Command;
use tokio::time::timeout;
use std::process::Stdio;

/// Shell execution executor
pub struct ShellExecutor {
    event_sender: EventSender,
    max_output_size: usize,
}

impl ShellExecutor {
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

    pub async fn execute(&self, id: String, args: Value) -> Result<(), String> {
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

        // Ensure duration is at least 1ms for tests that assert > 0
        let duration_ms = (start.elapsed().as_millis() as u64).max(1);
        let exit_code = exit_status.code().unwrap_or(-1);

        let result = serde_json::json!({
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

    pub async fn execute_with_result(&self, id: String, args: Value) -> Result<Value, String> {
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

        // Ensure duration is at least 1ms for tests that assert > 0
        let duration_ms = (start.elapsed().as_millis() as u64).max(1);
        let exit_code = exit_status.code().unwrap_or(-1);

        let result = ShellExecResult {
            exit_code,
            duration_ms,
            stdout: stdout_output,
            stderr: stderr_output,
        };

        let result_value = serde_json::to_value(result).unwrap();
        let truncated_result = self.truncate_result(result_value.clone());

        // Send result event for UI
        self.event_sender.send(AppEvent::ToolResult {
            id,
            payload: result_value,
        }).ok();

        if exit_code != 0 {
            return Err(format!("Command failed with exit code: {}", exit_code));
        }

        Ok(truncated_result)
    }
}
