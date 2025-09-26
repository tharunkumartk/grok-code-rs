pub mod agent;
pub mod events;
pub mod session;
pub mod tools;

// Re-export main types for convenience
pub use agent::{Agent, AgentResponse, AgentError, AgentFactory};
pub use events::{AppEvent, EventBus, Request, ToolName, ToolSpec, TokenUsage};
pub use session::{Session, ChatMessage, MessageRole, ToolStatus, ToolMessageInfo};
pub use tools::{ToolExecutor, ToolRegistry};
