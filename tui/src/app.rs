use anyhow::Result;
use crossterm::event;
use grok_core::{AppEvent, Session};
use ratatui::{backend::Backend, Frame, Terminal};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::info;

use crate::{
    components::{ChatComponent, InputComponent, ToolsComponent, StatusComponent, CommandPaletteComponent},
    handlers::{InputHandler, EventHandler},
    state::AppState,
    utils::{layout, terminal},
};

/// Main application
pub struct App {
    state: AppState,
}

impl App {
    /// Create a new application instance
    pub fn new(
        session: Session,
        event_receiver: mpsc::UnboundedReceiver<AppEvent>,
    ) -> Self {
        let chats_dir = Session::default_history_path().parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("chats");
        Self {
            state: AppState::new(session, event_receiver, chats_dir),
        }
    }
    
    /// Run the application main loop
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        let mut terminal = terminal::setup()?;
        
        info!("TUI initialized, starting main loop");
        
        // Main application loop
        let result = self.run_app(&mut terminal).await;
        
        // Restore terminal
        terminal::restore(&mut terminal)?;
        
        result
    }
    
    /// Main application loop
    async fn run_app<B: Backend + std::io::Write>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            // Update cursor blinking
            self.state.update_cursor_blink();

            // Draw UI
            terminal.draw(|f| self.ui(f))?;

            // Handle events with timeout to ensure UI responsiveness
            tokio::select! {
                // Handle terminal events (keyboard input)
                terminal_event = async {
                    if event::poll(Duration::from_millis(0)).unwrap_or(false) {
                        event::read().ok()
                    } else {
                        None
                    }
                } => {
                    if let Some(event) = terminal_event {
                        InputHandler::handle_event(&mut self.state, event).await;
                    }
                },

                // Handle application events (agent responses, etc.)
                app_event = self.state.event_receiver.recv() => {
                    if let Some(event) = app_event {
                        EventHandler::handle_event(&mut self.state, event).await;
                    }
                },

                // Timeout to ensure regular UI updates
                _ = tokio::time::sleep(Duration::from_millis(50)) => {},
            }

            if self.state.should_quit {
                break;
            }
        }

        // Auto-save on exit if there's history
        if !self.state.session.messages().is_empty() {
            let _ = self.state.session.save();
        }

        Ok(())
    }
    
    /// Draw the user interface
    fn ui(&mut self, f: &mut Frame) {
        let main_chunks = layout::create_main_layout(f.size());

        // Top panel: Chat + Tools side by side
        let top_chunks = layout::create_top_panel_layout(main_chunks[0]);

        // Render components
        ChatComponent::render(&mut self.state, f, top_chunks[0]);
        ToolsComponent::render(&mut self.state, f, top_chunks[1]);
        InputComponent::render(&mut self.state, f, main_chunks[1]);
        StatusComponent::render(&self.state, f, main_chunks[2]);

        // Command palette overlay (render on top)
        if self.state.command_palette_open {
            CommandPaletteComponent::render(&mut self.state, f);
        }
    }
}