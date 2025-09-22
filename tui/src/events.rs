// Event handling utilities for the TUI
// This module will be expanded as we add more complex event handling

use crossterm::event::Event;

/// TUI-specific events
#[derive(Debug)]
pub enum TuiEvent {
    /// Terminal input event
    Input(()),
}

/// Convert crossterm events to our internal event type
impl From<Event> for TuiEvent {
    fn from(_event: Event) -> Self {
        TuiEvent::Input(())
    }
}
