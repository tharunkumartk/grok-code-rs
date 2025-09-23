use anyhow::Result;
use grok_core::{AgentFactory, EventBus, Session};
// use std::time::Duration;
use tracing::info;

mod app;
mod components;
mod events;
mod handlers;
pub mod markdown;
mod state;
mod utils;

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

    // Create OpenRouter agent (requires OPENROUTER_API_KEY)
    let agent = match AgentFactory::create_openrouter_from_env(event_sender.clone()) {
        Ok(agent) => agent,
        Err(e) => {
            eprintln!("Error: {}. Make sure OPENROUTER_API_KEY environment variable is set.", e);
            eprintln!("Get an API key from: https://openrouter.ai/keys");
            std::process::exit(1);
        }
    };
    
    // Create session
    let session = Session::new(agent, event_sender.clone());
    
    // Create and run the TUI application
    let mut app = App::new(session, event_bus.into_receiver());
    app.run().await?;
    
    info!("Grok Code TUI shutting down");
    Ok(())
}
