use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime};
use thiserror::Error;

pub mod agent_logic;

/// Main agent trait that all agent implementations must satisfy
#[async_trait]
pub trait Agent: Send + Sync {
    /// Submit a message and get a response
    async fn submit(
        &self,
        message: String,
        history: Vec<crate::session::ChatMessage>,
    ) -> Result<AgentResponse, AgentError>;
    
    /// Get agent information
    fn info(&self) -> AgentInfo;
}

/// Response from an agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub content: String,
    pub metadata: ResponseMetadata,
}

/// Metadata about the response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseMetadata {
    pub processing_time: Option<Duration>,
    pub tokens_used: Option<u32>,
    pub model: Option<String>,
    pub timestamp: SystemTime,
}

impl ResponseMetadata {
    pub fn new() -> Self {
        Self {
            processing_time: None,
            tokens_used: None,
            model: None,
            timestamp: SystemTime::now(),
        }
    }
    
    pub fn with_processing_time(mut self, duration: Duration) -> Self {
        self.processing_time = Some(duration);
        self
    }
}

/// Information about an agent
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub name: String,
    pub description: String,
    pub version: String,
}

/// Errors that can occur during agent operations
#[derive(Error, Debug, Clone)]
pub enum AgentError {
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Agent configuration error: {0}")]
    Configuration(String),
    
    #[error("Processing error: {0}")]
    Processing(String),
    
    #[error("Agent unavailable: {0}")]
    Unavailable(String),
}

/// Factory for creating different types of agents
pub struct AgentFactory;

impl AgentFactory {
    /// Create a multi-model agent with OpenRouter as primary and optional Vercel AI Gateway fallback.
    /// Required: OPENROUTER_API_KEY
    /// Optional: OPENROUTER_MODEL (default: "x-ai/grok-4-fast:free")
    /// Optional fallback: VERCEL_AI_GATEWAY_API_KEY, VERCEL_AI_GATEWAY_MODEL
    pub fn create_openrouter_from_env(
        event_sender: crate::events::EventSender,
    ) -> Result<std::sync::Arc<dyn Agent>, AgentError> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| AgentError::Configuration("Missing OPENROUTER_API_KEY".to_string()))?;
        let model = std::env::var("OPENROUTER_MODEL").unwrap_or_else(|_| "x-ai/grok-4-fast:free".to_string());

        let agent = agent_logic::MultiModelAgent::new(api_key, model, event_sender)
            .map_err(|e| AgentError::Configuration(format!("{}", e)))?;
        Ok(std::sync::Arc::new(agent))
    }
}
