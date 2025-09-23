use crate::agent::Agent;
use crate::events::{EventSender, ToolName};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

/// Represents a chat session with conversation history
pub struct Session {
    messages: Vec<ChatMessage>,
    agent: std::sync::Arc<dyn Agent>,
    event_sender: EventSender,
    active_tools: std::collections::HashMap<String, ActiveTool>,
}

/// Information about an active tool execution
#[derive(Debug, Clone)]
pub struct ActiveTool {
    pub id: String,
    pub tool: ToolName,
    pub summary: String,
    pub start_time: SystemTime,
    pub status: ToolStatus,
    pub stdout: String,
    pub stderr: String,
    pub result: Option<serde_json::Value>,
}

/// Status of a tool execution
#[derive(Debug, Clone, PartialEq)]
pub enum ToolStatus {
    Running,
    Completed,
    Failed,
}

/// A single message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: SystemTime,
}

/// Who sent the message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    User,
    Agent,
    System,
    Error,
}

impl Session {
    /// Create a new session with the given agent
    pub fn new(agent: std::sync::Arc<dyn Agent>, event_sender: EventSender) -> Self {
        let mut session = Self {
            messages: Vec::new(),
            agent,
            event_sender,
            active_tools: std::collections::HashMap::new(),
        };
        
        // Add welcome message
        session.add_system_message("Welcome to Grok Code! Type your message and press Enter.".to_string());
        
        session
    }
    
    /// Get all messages in the session
    pub fn messages(&self) -> &[ChatMessage] {
        &self.messages
    }
    
    /// Add a user message and process it with the agent
    pub async fn handle_user_input(&mut self, input: String) {
        // Add user message to history immediately for UI display
        self.add_user_message(input.clone());

        // Spawn background task to fetch agent response without blocking UI redraw
        let agent = self.agent.clone();
        let sender = self.event_sender.clone();
        let history = self.messages.clone();
        tokio::spawn(async move {
            match agent.submit(input, history).await {
                Ok(response) => {
                    let _ = sender.send_agent_response(response);
                }
                Err(error) => {
                    let _ = sender.send_agent_error(error);
                }
            }
        });
    }
    
    /// Add a user message to the conversation
    pub fn add_user_message(&mut self, content: String) {
        let message = ChatMessage {
            role: MessageRole::User,
            content,
            timestamp: SystemTime::now(),
        };
        self.messages.push(message);
    }
    
    /// Add an agent message to the conversation
    pub fn add_agent_message(&mut self, content: String) {
        let message = ChatMessage {
            role: MessageRole::Agent,
            content,
            timestamp: SystemTime::now(),
        };
        self.messages.push(message);
    }
    
    /// Add a system message to the conversation
    pub fn add_system_message(&mut self, content: String) {
        let message = ChatMessage {
            role: MessageRole::System,
            content,
            timestamp: SystemTime::now(),
        };
        self.messages.push(message);
    }
    
    /// Add an error message to the conversation
    pub fn add_error_message(&mut self, content: String) {
        let message = ChatMessage {
            role: MessageRole::Error,
            content,
            timestamp: SystemTime::now(),
        };
        self.messages.push(message);
    }
    
    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
        self.add_system_message("Conversation cleared.".to_string());
    }
    
    /// Get agent information
    pub fn agent_info(&self) -> crate::agent::AgentInfo {
        self.agent.info()
    }

    /// Get active tools
    pub fn active_tools(&self) -> &std::collections::HashMap<String, ActiveTool> {
        &self.active_tools
    }

    /// Handle tool begin event
    pub fn handle_tool_begin(&mut self, id: String, tool: ToolName, summary: String) {
        let active_tool = ActiveTool {
            id: id.clone(),
            tool,
            summary,
            start_time: SystemTime::now(),
            status: ToolStatus::Running,
            stdout: String::new(),
            stderr: String::new(),
            result: None,
        };
        self.active_tools.insert(id, active_tool);
    }

    /// Handle tool progress event
    pub fn handle_tool_progress(&mut self, id: String, _message: String) {
        // Progress events don't need to update stored state for now
        // They're handled by the UI directly
        if let Some(_tool) = self.active_tools.get_mut(&id) {
            // Could store progress messages if needed
        }
    }

    /// Handle tool stdout event
    pub fn handle_tool_stdout(&mut self, id: String, chunk: String) {
        if let Some(tool) = self.active_tools.get_mut(&id) {
            tool.stdout.push_str(&chunk);
        }
    }

    /// Handle tool stderr event
    pub fn handle_tool_stderr(&mut self, id: String, chunk: String) {
        if let Some(tool) = self.active_tools.get_mut(&id) {
            tool.stderr.push_str(&chunk);
        }
    }

    /// Handle tool result event
    pub fn handle_tool_result(&mut self, id: String, payload: serde_json::Value) {
        if let Some(tool) = self.active_tools.get_mut(&id) {
            tool.result = Some(payload);
        }
    }

    /// Handle tool end event
    pub fn handle_tool_end(&mut self, id: String, ok: bool, _duration_ms: u64) {
        if let Some(tool) = self.active_tools.get_mut(&id) {
            tool.status = if ok { ToolStatus::Completed } else { ToolStatus::Failed };
        }
    }
}

impl ChatMessage {
    /// Format the message for display
    pub fn formatted_content(&self) -> String {
        match self.role {
            MessageRole::User => format!("You: {}", self.content),
            MessageRole::Agent => format!("Agent: {}", self.content),
            MessageRole::System => format!("System: {}", self.content),
            MessageRole::Error => format!("Error: {}", self.content),
        }
    }
    
    /// Get the display color for this message role
    pub fn role_color(&self) -> &'static str {
        match self.role {
            MessageRole::User => "blue",
            MessageRole::Agent => "green",
            MessageRole::System => "yellow",
            MessageRole::Error => "red",
        }
    }
}

// TODO: Add tests back when we have a test agent implementation
// The current test was tightly coupled to MockAgent behavior
