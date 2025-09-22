use anyhow::Result;
use grok_core::{AgentFactory, EventBus, Session};
// use std::time::Duration;
use tracing::info;

mod app;
mod components;
mod events;

use app::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing - only log to stderr and filter out less important messages
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_writer(std::io::stderr)
        .init();
    info!("Starting Grok Code TUI");
    
    // Create event bus for communication
    let event_bus = EventBus::new();
    let event_sender = event_bus.sender();
    
    // Optional: load .env (ignore errors if missing)
    let _ = dotenvy::dotenv();

    // Choose agent: OpenRouter if API key present, else mock
    let agent = match std::env::var("OPENROUTER_API_KEY") {
        Ok(_) => AgentFactory::create_openrouter_from_env(event_sender.clone())
            .unwrap_or_else(|_| AgentFactory::create_mock_with_events(event_sender.clone())),
        Err(_) => AgentFactory::create_mock_with_events(event_sender.clone()),
    };
    
    // Create session
    let session = Session::new(agent, event_sender.clone());
    
    // Create and run the TUI application
    let mut app = App::new(session, event_bus.into_receiver());
    app.run().await?;
    
    info!("Grok Code TUI shutting down");
    Ok(())
}
