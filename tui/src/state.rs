use grok_core::{AppEvent, Session, TokenUsage, ChatMessage, MessageRole};
use std::time::Instant;
use tokio::sync::mpsc;
use std::path::PathBuf;
use std::fs;
use std::time::SystemTime;
use serde::{Deserialize, Serialize};
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInfo {
    pub title: String,
    pub path: PathBuf,
    pub last_modified: SystemTime,
}

/// Command for the command palette
#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub description: String,
}

pub fn scan_chats(dir: &PathBuf) -> Result<Vec<ChatInfo>> {
    fs::create_dir_all(dir)?;
    let mut chats = vec![];
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |e| e == "json") {
                if let Ok(contents) = fs::read_to_string(&path) {
                    if let Ok(messages) = serde_json::from_str::<Vec<ChatMessage>>(&contents) {
                        if !messages.is_empty() {
                            let title = messages.iter()
                                .find(|m| m.role == MessageRole::User)
                                .map(|m| sanitize_filename(&m.content))
                                .unwrap_or_else(|| {
                                    path.file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("Untitled")
                                        .to_string()
                                });
                            if let Ok(meta) = fs::metadata(&path) {
                                if let Ok(last_mod) = meta.modified() {
                                    chats.push(ChatInfo {
                                        title,
                                        path,
                                        last_modified: last_mod,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    chats.sort_by(|a, b| b.last_modified.cmp(&a.last_modified));
    Ok(chats)
}

pub fn sanitize_filename(name: &str) -> String {
    name.chars()
        .take(50)
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .trim()
        .to_string()
        .replace(' ', "_")
}

pub fn save_chat(session: &Session, path: &PathBuf) -> Result<()> {
    let contents = serde_json::to_string_pretty(&session.messages())?;
    fs::write(path, contents)?;
    Ok(())
}

pub fn load_chat(path: &PathBuf) -> Result<Vec<ChatMessage>> {
    let contents = fs::read_to_string(path)?;
    let messages: Vec<ChatMessage> = serde_json::from_str(&contents)?;
    Ok(messages)
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

    /// Directory for chat files
    pub chats_dir: PathBuf,

    /// List of available chats
    pub available_chats: Vec<ChatInfo>,

    /// Whether to show the chat selection list
    pub show_chat_list: bool,

    /// Path to the current chat file
    pub current_chat_path: Option<PathBuf>,

    /// Selected chat index in the list
    pub selected_chat_index: usize,

    /// Dirty flag for autosave
    pub dirty: bool,
}

impl AppState {
    /// Create a new application state
    pub fn new(session: Session, event_receiver: mpsc::UnboundedReceiver<AppEvent>, chats_dir: PathBuf) -> Self {
        let available_commands = vec![
            Command {
                name: "/context".to_string(),
                description: "Show current token usage information".to_string(),
            },
            Command {
                name: "/quit".to_string(),
                description: "Exit the application".to_string(),
            },
            Command {
                name: "/clear".to_string(),
                description: "Clear conversation history and start new chat".to_string(),
            },
            Command {
                name: "/info".to_string(),
                description: "Show agent information".to_string(),
            },
            Command {
                name: "/new".to_string(),
                description: "Start a new chat".to_string(),
            },
            Command {
                name: "/save".to_string(),
                description: "Save current chat with a title based on first message".to_string(),
            },
            Command {
                name: "/load".to_string(),
                description: "Load a specific chat (use chat list)".to_string(),
            },
        ];

        let available_chats = scan_chats(&chats_dir).unwrap_or_default();
        let show_chat_list = !available_chats.is_empty() && session.messages().is_empty();

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
            focused_panel: if show_chat_list { 1 } else { 0 },
            auto_scroll_chat: true,
            auto_scroll_tools: true,
            cursor_visible: true,
            last_cursor_blink: Instant::now(),
            command_palette_open: false,
            command_palette_selected: 0,
            command_palette_filter: String::new(),
            available_commands,
            current_token_usage: None,
            chats_dir,
            available_chats,
            show_chat_list,
            current_chat_path: None,
            selected_chat_index: 0,
            dirty: false,
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

    /// Autosave the current chat if path is set and dirty
    pub fn autosave(&mut self) -> Result<()> {
        if self.dirty && self.current_chat_path.is_some() {
            save_chat(&self.session, &self.current_chat_path.as_ref().unwrap())?;
            self.dirty = false;
            // Refresh available_chats to include new/updated
            self.available_chats = scan_chats(&self.chats_dir)?;
        }
        Ok(())
    }

    /// Set dirty flag
    pub fn set_dirty(&mut self) {
        self.dirty = true;
    }

    /// Load a chat by index
    pub fn load_chat_by_index(&mut self, index: usize) -> Result<()> {
        if let Some(chat) = self.available_chats.get(index) {
            let messages = load_chat(&chat.path)?;
            self.session.replace_messages(messages);
            self.current_chat_path = Some(chat.path.clone());
            self.show_chat_list = false;
            self.chat_scroll = 0;
            self.set_dirty();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Invalid chat index"))
        }
    }

    /// Start new chat
    pub fn start_new_chat(&mut self) {
        self.session.clear();
        self.current_chat_path = None;
        self.dirty = false;
        self.show_chat_list = !self.available_chats.is_empty();
        self.chat_scroll = 0;
        if self.show_chat_list {
            self.focused_panel = 1;
        }
    }

    /// Save current chat with title from first message
    pub fn save_current_chat(&mut self) -> Result<()> {
        if self.session.messages().is_empty() {
            return Ok(());
        }
        let first_user_msg = self.session.messages().iter()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone());
        let title = first_user_msg
            .map(|s| sanitize_filename(&s))
            .unwrap_or_else(|| "Untitled".to_string());
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0u64, |d| d.as_secs());
        let filename = format!("{}_{}.json", title, timestamp);
        let path = self.chats_dir.join(filename);
        save_chat(&self.session, &path)?;
        self.current_chat_path = Some(path.clone());
        self.set_dirty();
        // Refresh list
        self.available_chats = scan_chats(&self.chats_dir)?;
        self.show_chat_list = false;
        Ok(())
    }
}