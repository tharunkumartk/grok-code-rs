use super::{Agent, AgentError, AgentInfo, AgentResponse, ResponseMetadata};
use crate::events::{EventSender, ToolName};
use crate::tools::ToolExecutor;
use async_trait::async_trait;
use serde_json::json;
use std::time::{Duration, Instant};
use uuid::Uuid;

/// Mock agent that echoes back the input with a simulated delay
pub struct MockAgent {
    info: AgentInfo,
    delay: Duration,
    event_sender: Option<EventSender>,
}

impl MockAgent {
    pub fn new() -> Self {
        Self {
            info: AgentInfo {
                name: "Mock Agent".to_string(),
                description: "A tool-calling agent for testing UI".to_string(),
                version: "0.1.0".to_string(),
            },
            delay: Duration::from_millis(300), // Simulate processing time
            event_sender: None,
        }
    }
    
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn with_event_sender(mut self, event_sender: EventSender) -> Self {
        self.event_sender = Some(event_sender);
        self
    }
}

impl Default for MockAgent {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Agent for MockAgent {
    async fn submit(&self, message: String, _history: Vec<crate::session::ChatMessage>) -> Result<AgentResponse, AgentError> {
        let start = Instant::now();
        
        // Simulate potential errors for testing
        if message.trim().eq_ignore_ascii_case("error") {
            return Err(AgentError::Processing("Simulated error".to_string()));
        }
        
        if message.trim().eq_ignore_ascii_case("network error") {
            return Err(AgentError::Network("Simulated network failure".to_string()));
        }

        // If we have an event sender, simulate tool calling behavior
        if let Some(ref event_sender) = self.event_sender {
            return self.simulate_tool_calling_response(&message, event_sender).await;
        }

        // Fallback to simple echo behavior
        tokio::time::sleep(self.delay).await;
        let processing_time = start.elapsed();
        
        let response = AgentResponse {
            content: format!("I'm going to help you with: {}", message),
            metadata: ResponseMetadata::new()
                .with_processing_time(processing_time),
        };
        
        Ok(response)
    }
    
    fn info(&self) -> AgentInfo {
        self.info.clone()
    }
}

impl MockAgent {
    /// Simulate a tool-calling response based on the user's message
    async fn simulate_tool_calling_response(&self, message: &str, event_sender: &EventSender) -> Result<AgentResponse, AgentError> {
        let start = Instant::now();
        
        // Send chat creation event
        use crate::events::AppEvent;
        let _ = event_sender.send(AppEvent::ChatCreated);
        
        // Stream initial response
        let _ = event_sender.send(AppEvent::ChatDelta { 
            text: "I'll help you with that. Let me use some tools to analyze your request.\n\n".to_string() 
        });
        
        tokio::time::sleep(Duration::from_millis(200)).await;
        
        // Create tool executor for running tools
        let executor = ToolExecutor::new(event_sender.clone());
        
        // Determine which tools to call based on message content
        let tools_to_call = self.determine_tools_for_message(message);
        
        for (tool_name, args) in tools_to_call {
            let tool_id = Uuid::new_v4().to_string();
            
            // Send chat delta about using the tool
            let tool_description = match tool_name {
                ToolName::FsRead => "reading file",
                ToolName::FsSearch => "searching files",
                ToolName::FsWrite => "writing file",
                ToolName::FsApplyPatch => "applying patch",
                ToolName::ShellExec => "executing command",
            };
            
            let _ = event_sender.send(AppEvent::ChatDelta { 
                text: format!("Now I'm {} to help with your request...\n", tool_description)
            });
            
            // Execute the tool
            if let Err(e) = executor.execute_tool(tool_id, tool_name, args).await {
                // Log the error but don't send a UI message to avoid clutter
                tracing::error!("Tool execution failed: {}", e);
            }
            
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        
        // Send final response
        let _ = event_sender.send(AppEvent::ChatDelta { 
            text: "\nAll tools have completed successfully! I've processed your request using the appropriate tools.".to_string() 
        });
        
        let _ = event_sender.send(AppEvent::ChatCompleted { 
            token_usage: Some(crate::events::TokenUsage {
                input_tokens: message.len() as u32,
                output_tokens: 150,
                total_tokens: message.len() as u32 + 150,
            })
        });
        
        let processing_time = start.elapsed();
        
        let response = AgentResponse {
            content: format!("I've processed your request: \"{}\" using various tools. Check the tool outputs above for details.", message),
            metadata: ResponseMetadata::new()
                .with_processing_time(processing_time),
        };
        
        Ok(response)
    }
    
    /// Determine which tools to call based on the message content
    fn determine_tools_for_message(&self, message: &str) -> Vec<(ToolName, serde_json::Value)> {
        let message_lower = message.to_lowercase();
        let mut tools = Vec::new();
        
        // Read file examples
        if message_lower.contains("read") || message_lower.contains("show") || message_lower.contains("cat") {
            tools.push((ToolName::FsRead, json!({
                "path": "src/main.rs",
                "encoding": "utf-8"
            })));
        }
        
        // Search examples
        if message_lower.contains("search") || message_lower.contains("find") || message_lower.contains("grep") {
            let query = if message_lower.contains("function") { "function" }
                       else if message_lower.contains("struct") { "struct" }
                       else if message_lower.contains("impl") { "impl" }
                       else { "TODO" };
            
            tools.push((ToolName::FsSearch, json!({
                "query": query,
                "regex": false,
                "case_insensitive": true,
                "multiline": false
            })));
        }
        
        // Write examples
        if message_lower.contains("write") || message_lower.contains("create") || message_lower.contains("save") {
            tools.push((ToolName::FsWrite, json!({
                "path": "output.txt",
                "contents": format!("Generated content based on: {}", message),
                "create_if_missing": true,
                "overwrite": false
            })));
        }
        
        // Patch examples
        if message_lower.contains("patch") || message_lower.contains("diff") || message_lower.contains("apply") {
            tools.push((ToolName::FsApplyPatch, json!({
                "unified_diff": "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,3 @@\n line1\n-old line\n+new line\n line3",
                "dry_run": message_lower.contains("dry")
            })));
        }
        
        // Shell exec examples
        if message_lower.contains("run") || message_lower.contains("execute") || message_lower.contains("command") || message_lower.contains("build") {
            let command = if message_lower.contains("build") { vec!["cargo".to_string(), "build".to_string()] }
                         else if message_lower.contains("test") { vec!["cargo".to_string(), "test".to_string()] }
                         else { vec!["echo".to_string(), "Hello from shell".to_string()] };
            
            tools.push((ToolName::ShellExec, json!({
                "command": command,
                "cwd": ".",
                "timeout_ms": 10000
            })));
        }
        
        // If no specific tools were triggered, do a demo sequence
        if tools.is_empty() {
            tools.extend([
                (ToolName::FsRead, json!({
                    "path": "README.md",
                    "encoding": "utf-8"
                })),
                (ToolName::FsSearch, json!({
                    "query": "grok",
                    "regex": false,
                    "case_insensitive": true
                })),
                (ToolName::ShellExec, json!({
                    "command": ["echo", "Demo completed successfully!"],
                    "cwd": ".",
                    "timeout_ms": 5000
                })),
            ]);
        }
        
        tools
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio_test;
    
    #[tokio::test]
    async fn test_mock_agent_echo() {
        let agent = MockAgent::new().with_delay(Duration::from_millis(10));
        let response = agent.submit("Hello, world!".to_string(), vec![]).await.unwrap();
        
        assert!(response.content.contains("I'm going to help you with:"));
        assert!(response.metadata.processing_time.is_some());
    }
    
    #[tokio::test]
    async fn test_mock_agent_error() {
        let agent = MockAgent::new().with_delay(Duration::from_millis(10));
        let result = agent.submit("error".to_string(), vec![]).await;
        
        assert!(result.is_err());
        match result.unwrap_err() {
            AgentError::Processing(msg) => assert_eq!(msg, "Simulated error"),
            _ => panic!("Expected processing error"),
        }
    }
}
