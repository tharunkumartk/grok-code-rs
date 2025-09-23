/// Utility functions for the TUI application

/// Terminal management utilities
pub mod terminal {
    use anyhow::Result;
    use crossterm::{
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        event::{DisableMouseCapture, EnableMouseCapture},
    };
    use ratatui::{backend::CrosstermBackend, Terminal};
    use std::io;

    /// Setup terminal for TUI mode
    pub fn setup() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(terminal)
    }

    /// Restore terminal to normal mode
    pub fn restore<B: ratatui::backend::Backend + std::io::Write>(terminal: &mut Terminal<B>) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        Ok(())
    }
}

/// Layout calculation utilities
pub mod layout {
    use ratatui::layout::{Constraint, Direction, Layout, Rect};

    /// Create the main application layout
    pub fn create_main_layout(area: Rect) -> Vec<Rect> {
        Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Min(1),     // Chat area
                Constraint::Length(5),  // Input area (increased for multi-line)
                Constraint::Length(1),  // Status line
            ].as_ref())
            .split(area)
            .to_vec()
    }

    /// Create the top panel layout (chat + tools)
    pub fn create_top_panel_layout(area: Rect) -> Vec<Rect> {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(60), // Chat area
                Constraint::Percentage(40), // Tools area
            ].as_ref())
            .split(area)
            .to_vec()
    }
}
