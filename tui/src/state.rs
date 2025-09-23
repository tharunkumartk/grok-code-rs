use grok_core::{AppEvent, Session, TokenUsage};
use std::time::Instant;
use tokio::sync::mpsc;

/// Command for the command palette
#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub description: String,
    pub syntax: String,
}

/// Application state
pub struct AppState {
    /// The chat session
    pub session: Session,

    /// Current input text
    pub input: String,

    /// Cursor position in input text (byte index)
    pub input_cursor: usize,

    /// Whether the application should quit
    pub should_quit: bool,

    /// Whether we're waiting for an agent response
    pub processing: bool,

    /// Event receiver for handling app events
    pub event_receiver: mpsc::UnboundedReceiver<AppEvent>,

    /// Chat scroll state
    pub chat_scroll: usize,

    /// Tools scroll state
    pub tools_scroll: usize,

    /// Input scroll state (for multi-line input)
    pub input_scroll: usize,

    /// Currently focused panel (0 = chat input, 1 = chat history, 2 = tools)
    pub focused_panel: usize,

    /// Whether to auto-scroll chat to bottom on new messages
    pub auto_scroll_chat: bool,

    /// Whether to auto-scroll tools to bottom on new tools/updates
    pub auto_scroll_tools: bool,

    /// Whether cursor is visible (for blinking effect)
    pub cursor_visible: bool,

    /// Last time cursor blinked
    pub last_cursor_blink: Instant,

    /// Command palette state
    pub command_palette_open: bool,

    /// Currently selected command in palette
    pub command_palette_selected: usize,

    /// Filter text for command palette
    pub command_palette_filter: String,

    /// Available commands
    pub available_commands: Vec<Command>,

    /// Current token usage total
    pub current_token_usage: Option<TokenUsage>,
}

impl AppState {
    /// Create a new application state
    pub fn new(session: Session, event_receiver: mpsc::UnboundedReceiver<AppEvent>) -> Self {
        // Define available commands
        let available_commands = vec![
            Command {
                name: "/context".to_string(),
                description: "Show current token usage information".to_string(),
                syntax: "/context".to_string(),
            },
            Command {
                name: "/quit".to_string(),
                description: "Exit the application".to_string(),
                syntax: "/quit or /q".to_string(),
            },
            Command {
                name: "/clear".to_string(),
                description: "Clear conversation history".to_string(),
                syntax: "/clear".to_string(),
            },
            Command {
                name: "/info".to_string(),
                description: "Show agent information".to_string(),
                syntax: "/info".to_string(),
            },
        ];

        Self {
            session,
            input: String::new(),
            input_cursor: 0,
            should_quit: false,
            processing: false,
            event_receiver,
            chat_scroll: 0,
            tools_scroll: 0,
            input_scroll: 0,
            focused_panel: 0,
            auto_scroll_chat: true,
            auto_scroll_tools: true,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            command_palette_open: false,
            command_palette_selected: 0,
            command_palette_filter: String::new(),
            available_commands,
            current_token_usage: None,
        }
    }

    /// Update cursor blinking state
    pub fn update_cursor_blink(&mut self) {
        let now = Instant::now();
        if now.duration_since(self.last_cursor_blink).as_millis() >= 500 {
            self.cursor_visible = !self.cursor_visible;
            self.last_cursor_blink = now;
        }
    }
}
