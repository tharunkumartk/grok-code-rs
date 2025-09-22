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
    info!("Starting Grok Code CLI");
    
    // For now, just launch the TUI
    // In the future, this could parse command line arguments
    // and decide whether to run in TUI mode, headless mode, etc.
    
    // Create event bus for communication
    let event_bus = EventBus::new();
    let event_sender = event_bus.sender();
    
    // Create mock agent
    let agent = AgentFactory::create_mock();
    
    // Create session
    let session = Session::new(agent, event_sender.clone());
    
    // Create and run the TUI application
    let mut app = grok_tui::App::new(session, event_bus.into_receiver());
    app.run().await?;
    
    info!("Grok Code CLI shutting down");
    Ok(())
}
