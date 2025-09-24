use anyhow::Result;
use grok_core::{AgentFactory, EventBus, Session};
use std::env;
use std::io::{self, Write};
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

    // Check for OpenRouter API key and prompt if missing
    if env::var("OPENROUTER_API_KEY").is_err() {
        println!("OpenRouter API key not found in environment.");
        println!("Get one from: https://openrouter.ai/keys");
        print!("Enter your API key: ");
        io::stdout().flush()?;
        
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let key = input.trim().to_string();
        
        if key.is_empty() {
            eprintln!("Error: API key cannot be empty.");
            std::process::exit(1);
        }
        
        env::set_var("OPENROUTER_API_KEY", key);
        println!("API key set. Proceeding...");
    }

    // Create OpenRouter agent (now with key guaranteed to be set)
    let agent = match AgentFactory::create_openrouter_from_env(event_sender.clone()) {
        Ok(agent) => agent,
        Err(e) => {
            eprintln!("Error creating agent: {}. Please check your API key.", e);
            std::process::exit(1);
        }
    };
    
    // Create session
    let mut session = Session::new(agent, event_sender.clone());
    
    // Check for previous history and notify user
    let history_path = Session::default_history_path();
    if history_path.exists() {
        session.add_system_message(format!(
            "Previous chat history found at {:?}. Use /load to restore it, or /clear to start fresh.",
            history_path
        ));
    }
    
    // Create and run the TUI application
    let mut app = App::new(session, event_bus.into_receiver());
    app.run().await?;
    
    info!("Grok Code TUI shutting down");
    Ok(())
}
