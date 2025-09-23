use crate::agent::{AgentError, AgentResponse};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Requests sent to core
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    ChatSubmit { text: String },
    ToolInvoke { id: String, tool: ToolName, args: serde_json::Value },
}

/// Events that flow through the application
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// User submitted input
    UserInput(String),
    
    /// Agent provided a response
    AgentResponse(AgentResponse),
    
    /// Agent encountered an error
    AgentError(AgentError),
    
    /// Agent thinking step (for interleaved thinking)
    AgentThinking(String),
    
    /// Application should quit
    Quit,
    
    /// Clear the conversation history
    Clear,
    
    /// Show agent information
    ShowAgentInfo,
    
    // Chat events
    ChatCreated,
    ChatDelta { text: String },
    ChatCompleted { token_usage: Option<TokenUsage> },

    // Tool lifecycle events
    ToolBegin { id: String, tool: ToolName, summary: String, args: Option<serde_json::Value> },
    ToolProgress { id: String, message: String },
    ToolStdout { id: String, chunk: String },
    ToolStderr { id: String, chunk: String },
    ToolResult { id: String, payload: serde_json::Value },
    ToolEnd { id: String, ok: bool, duration_ms: u64 },

    // Safety/approval/errors
    ApprovalRequest { id: String, tool: ToolName, summary: String },
    ApprovalDecision { id: String, approved: bool },
    Error { id: Option<String>, message: String },
    TokenCount(TokenUsage),
    Background(String),
}

/// Available tools
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ToolName {
    FsRead,
    FsSearch,
    FsWrite,
    FsApplyPatch,
    FsFind,
    ShellExec,
    CodeSymbols,
}

/// Token usage information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub total_tokens: u32,
}

/// Tool specification for discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: ToolName,
    pub input_schema: serde_json::Value,   // JSON Schema
    pub output_schema: serde_json::Value,  // JSON Schema
    pub streaming: bool,                   // supports stdout/stderr/progress
    pub side_effects: bool,                // mutates filesystem/environment
    pub needs_approval: bool,              // may trigger ApprovalRequest
    pub timeout_ms: Option<u64>,
}

/// Event bus for communication between components
#[derive(Debug)]
pub struct EventBus {
    sender: mpsc::UnboundedSender<AppEvent>,
    receiver: mpsc::UnboundedReceiver<AppEvent>,
}

impl EventBus {
    /// Create a new event bus
    pub fn new() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self { sender, receiver }
    }
    
    /// Get a sender handle for the event bus
    pub fn sender(&self) -> EventSender {
        EventSender {
            inner: self.sender.clone(),
        }
    }
    
    /// Get the receiver (should only be used by the main event loop)
    pub fn into_receiver(self) -> mpsc::UnboundedReceiver<AppEvent> {
        self.receiver
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// Handle for sending events to the event bus
#[derive(Debug, Clone)]
pub struct EventSender {
    inner: mpsc::UnboundedSender<AppEvent>,
}

impl EventSender {
    /// Send an event to the bus
    pub fn send(&self, event: AppEvent) -> Result<(), EventSendError> {
        self.inner
            .send(event)
            .map_err(|_| EventSendError::ChannelClosed)
    }
    
    /// Send user input
    pub fn send_user_input(&self, message: String) -> Result<(), EventSendError> {
        self.send(AppEvent::UserInput(message))
    }
    
    /// Send agent response
    pub fn send_agent_response(&self, response: AgentResponse) -> Result<(), EventSendError> {
        self.send(AppEvent::AgentResponse(response))
    }
    
    /// Send agent error
    pub fn send_agent_error(&self, error: AgentError) -> Result<(), EventSendError> {
        self.send(AppEvent::AgentError(error))
    }
    
    /// Send agent thinking step
    pub fn send_agent_thinking(&self, thinking: String) -> Result<(), EventSendError> {
        self.send(AppEvent::AgentThinking(thinking))
    }
    
    /// Send quit signal
    pub fn send_quit(&self) -> Result<(), EventSendError> {
        self.send(AppEvent::Quit)
    }
}

/// Errors that can occur when sending events
#[derive(Debug, thiserror::Error)]
pub enum EventSendError {
    #[error("Event channel is closed")]
    ChannelClosed,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_event_bus() {
        let bus = EventBus::new();
        let sender = bus.sender();
        let mut receiver = bus.into_receiver();
        
        // Send a test event
        sender.send_user_input("test message".to_string()).unwrap();
        
        // Receive the event
        let event = receiver.recv().await.unwrap();
        match event {
            AppEvent::UserInput(msg) => assert_eq!(msg, "test message"),
            _ => panic!("Expected UserInput event"),
        }
    }
}
