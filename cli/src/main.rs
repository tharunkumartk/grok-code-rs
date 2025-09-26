use anyhow::Result;
use grok_core::{AgentFactory, EventBus, Session};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing - only log to stderr and filter out less important messages
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_writer(std::io::stderr)
        .init();
    
    // For now, just launch the TUI
    // In the future, this could parse command line arguments
    // and decide whether to run in TUI mode, headless mode, etc.
    
    // Create event bus for communication
    let event_bus = EventBus::new();
    let event_sender = event_bus.sender();
    
    // Load environment variables
    let _ = dotenvy::dotenv();

    // Create OpenRouter agent (requires OPENROUTER_API_KEY)
    let agent = AgentFactory::create_openrouter_from_env(event_sender.clone())
        .map_err(|e| anyhow::anyhow!("Failed to create agent: {}. Make sure OPENROUTER_API_KEY is set.", e))?;
    
    // Create session
    let session = Session::new(agent, event_sender.clone());
    
    // Create and run the TUI application
    let mut app = grok_tui::App::new(session, event_bus.into_receiver());
    app.run().await?;
    
    Ok(())
}
