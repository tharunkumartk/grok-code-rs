use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use grok_core::{AppEvent, Session, ToolStatus};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Modifier},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame, Terminal,
};
use std::io;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

// use crate::components::{ChatDisplay, InputWidget};

/// Main application state
pub struct App {
    /// The chat session
    session: Session,

    /// Current input text
    input: String,

    /// Whether the application should quit
    should_quit: bool,

    /// Whether we're waiting for an agent response
    processing: bool,

    /// Event receiver for handling app events
    event_receiver: mpsc::UnboundedReceiver<AppEvent>,

    /// Chat scroll state
    chat_scroll: usize,

    /// Tools scroll state
    tools_scroll: usize,

    /// Currently focused panel (0 = chat, 1 = tools)
    focused_panel: usize,

    /// Whether to auto-scroll chat to bottom on new messages
    auto_scroll_chat: bool,

    /// Whether to auto-scroll tools to bottom on new tools/updates
    auto_scroll_tools: bool,
}

impl App {
    /// Create a new application instance
    pub fn new(
        session: Session,
        event_receiver: mpsc::UnboundedReceiver<AppEvent>,
    ) -> Self {
        Self {
            session,
            input: String::new(),
            should_quit: false,
            processing: false,
            event_receiver,
            chat_scroll: 0,
            tools_scroll: 0,
            focused_panel: 0,
            auto_scroll_chat: true,
            auto_scroll_tools: true,
        }
    }
    
    /// Run the application main loop
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        
        info!("TUI initialized, starting main loop");
        
        // Main application loop
        let result = self.run_app(&mut terminal).await;
        
        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;
        
        result
    }
    
    /// Main application loop
    async fn run_app<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            // Draw UI
            terminal.draw(|f| self.ui(f))?;
            
            // Handle events with timeout to ensure UI responsiveness
            tokio::select! {
                // Handle terminal events (keyboard input)
                terminal_event = async {
                    if event::poll(std::time::Duration::from_millis(0)).unwrap_or(false) {
                        event::read().ok()
                    } else {
                        None
                    }
                } => {
                    if let Some(Event::Key(key)) = terminal_event {
                        if key.kind == KeyEventKind::Press {
                            self.handle_key_event(key).await;
                        }
                    }
                },
                
                // Handle application events (agent responses, etc.)
                app_event = self.event_receiver.recv() => {
                    if let Some(event) = app_event {
                        self.handle_app_event(event).await;
                    }
                },
                
                // Timeout to ensure regular UI updates
                _ = tokio::time::sleep(tokio::time::Duration::from_millis(50)) => {},
            }
            
            if self.should_quit {
                break;
            }
        }
        
        Ok(())
    }
    
    /// Handle keyboard events
    async fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        match key.code {
            KeyCode::Char('q') if !self.processing => {
                self.should_quit = true;
            }
            KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Tab => {
                // Switch between panels
                self.focused_panel = (self.focused_panel + 1) % 2;
            }
            KeyCode::Up => {
                // Scroll up in focused panel
                match self.focused_panel {
                    0 => {
                        if self.chat_scroll > 0 {
                            self.chat_scroll = self.chat_scroll.saturating_sub(1);
                            // Disable auto-scroll when user manually scrolls
                            self.auto_scroll_chat = false;
                        }
                    },
                    1 => {
                        if self.tools_scroll > 0 {
                            self.tools_scroll = self.tools_scroll.saturating_sub(1);
                            // Disable auto-scroll when user manually scrolls
                            self.auto_scroll_tools = false;
                        }
                    },
                    _ => {}
                }
            }
            KeyCode::Down => {
                // Scroll down in focused panel
                // The render function will clamp to actual bounds
                match self.focused_panel {
                    0 => {
                        self.chat_scroll = self.chat_scroll.saturating_add(1);
                        // Disable auto-scroll when user manually scrolls
                        self.auto_scroll_chat = false;
                    },
                    1 => {
                        self.tools_scroll = self.tools_scroll.saturating_add(1);
                        // Disable auto-scroll when user manually scrolls
                        self.auto_scroll_tools = false;
                    },
                    _ => {}
                }
            }
            KeyCode::PageUp => {
                // Page up in focused panel
                match self.focused_panel {
                    0 => {
                        if self.chat_scroll >= 10 {
                            self.chat_scroll = self.chat_scroll.saturating_sub(10);
                        } else {
                            self.chat_scroll = 0;
                        }
                        // Disable auto-scroll when user manually scrolls
                        self.auto_scroll_chat = false;
                    },
                    1 => {
                        if self.tools_scroll >= 10 {
                            self.tools_scroll = self.tools_scroll.saturating_sub(10);
                        } else {
                            self.tools_scroll = 0;
                        }
                        // Disable auto-scroll when user manually scrolls
                        self.auto_scroll_tools = false;
                    },
                    _ => {}
                }
            }
            KeyCode::PageDown => {
                // Page down in focused panel
                // The render function will clamp to actual bounds
                match self.focused_panel {
                    0 => {
                        self.chat_scroll = self.chat_scroll.saturating_add(10);
                        // Disable auto-scroll when user manually scrolls
                        self.auto_scroll_chat = false;
                    },
                    1 => {
                        self.tools_scroll = self.tools_scroll.saturating_add(10);
                        // Disable auto-scroll when user manually scrolls
                        self.auto_scroll_tools = false;
                    },
                    _ => {}
                }
            }
            KeyCode::Enter if self.focused_panel == 0 => {
                self.submit_input().await;
            }
            KeyCode::Char(c) if self.focused_panel == 0 => {
                self.input.push(c);
            }
            KeyCode::Backspace if self.focused_panel == 0 => {
                self.input.pop();
            }
            KeyCode::Esc => {
                self.input.clear();
                self.focused_panel = 0; // Return focus to chat
            }
            KeyCode::End => {
                // Jump to bottom and re-enable auto-scroll for focused panel
                match self.focused_panel {
                    0 => {
                        self.auto_scroll_chat = true;
                    },
                    1 => {
                        self.auto_scroll_tools = true;
                    },
                    _ => {}
                }
            }
            _ => {}
        }
    }
    
    /// Handle application events
    async fn handle_app_event(&mut self, event: AppEvent) {
        debug!("Handling app event: {:?}", event);
        match event {
            AppEvent::UserInput(_) => {
                // User input is handled directly in submit_input
            }
            AppEvent::AgentResponse(response) => {
                // Append agent response and mark as done
                self.session.add_agent_message(response.content);
                self.processing = false;
                // Re-enable auto-scroll for new content
                self.auto_scroll_chat = true;
                debug!("Received agent response");
            }
            AppEvent::AgentError(error) => {
                self.session.add_error_message(format!("{}", error));
                self.processing = false;
                error!("Agent error: {}", error);
            }
            AppEvent::Quit => {
                self.should_quit = true;
            }
            AppEvent::Clear => {
                self.session.clear();
            }
            AppEvent::ShowAgentInfo => {
                let info = self.session.agent_info();
                self.session.add_system_message(format!(
                    "Agent: {} v{} - {}",
                    info.name, info.version, info.description
                ));
            }
            
            // Chat streaming events
            AppEvent::ChatCreated => {
                debug!("Chat created");
            }
            AppEvent::ChatDelta { text } => {
                // For now, accumulate chat deltas in the last agent message
                // In a more sophisticated implementation, you'd handle streaming differently
                debug!("Chat delta: {}", text);
            }
            AppEvent::ChatCompleted { token_usage } => {
                if let Some(usage) = token_usage {
                    debug!("Chat completed. Tokens used: {}", usage.total_tokens);
                }
                self.processing = false;
            }

            // Tool lifecycle events
            AppEvent::ToolBegin { id, tool, summary } => {
                debug!("Tool {} started: {}", id, summary);
                
                // Add a system message to chat panel indicating tool usage
                let tool_display_name = match tool {
                    grok_core::ToolName::FsRead => "file reader",
                    grok_core::ToolName::FsSearch => "file search",
                    grok_core::ToolName::FsWrite => "file writer",
                    grok_core::ToolName::FsApplyPatch => "patch applicator",
                    grok_core::ToolName::ShellExec => "shell command",
                };
                self.session.add_system_message(format!("Agent ran {} tool", tool_display_name));
                
                self.session.handle_tool_begin(id, tool, summary);
                // Re-enable auto-scroll for new tools and chat
                self.auto_scroll_tools = true;
                self.auto_scroll_chat = true;
            }
            AppEvent::ToolProgress { id, message } => {
                debug!("Tool {} progress: {}", id, message);
                self.session.handle_tool_progress(id, message);
            }
            AppEvent::ToolStdout { id, chunk } => {
                debug!("Tool {} stdout: {}", id, chunk);
                self.session.handle_tool_stdout(id, chunk);
            }
            AppEvent::ToolStderr { id, chunk } => {
                debug!("Tool {} stderr: {}", id, chunk);
                self.session.handle_tool_stderr(id, chunk);
            }
            AppEvent::ToolResult { id, payload } => {
                debug!("Tool {} result: {:?}", id, payload);
                self.session.handle_tool_result(id, payload);
            }
            AppEvent::ToolEnd { id, ok, duration_ms } => {
                debug!("Tool {} ended: ok={}, duration={}ms", id, ok, duration_ms);
                self.session.handle_tool_end(id, ok, duration_ms);
            }

            // Safety/approval events
            AppEvent::ApprovalRequest { id: _, tool, summary } => {
                debug!("Approval requested for tool {:?}: {}", tool, summary);
                // For mock implementation, auto-approve
                // In real implementation, show approval UI
                self.session.add_system_message(format!("Tool {:?} needs approval: {}", tool, summary));
            }
            AppEvent::ApprovalDecision { id, approved } => {
                debug!("Approval decision for {}: {}", id, approved);
            }

            // Error and background events
            AppEvent::Error { id: _, message } => {
                error!("Error: {}", message);
                self.session.add_error_message(format!("Error: {}", message));
            }
            AppEvent::TokenCount(usage) => {
                debug!("Token usage: {}/{} tokens", usage.input_tokens, usage.output_tokens);
            }
            AppEvent::Background(message) => {
                debug!("Background: {}", message);
            }
        }
    }
    
    /// Submit the current input to the agent
    async fn submit_input(&mut self) {
        if self.input.trim().is_empty() || self.processing {
            return;
        }
        
        let input = self.input.trim().to_string();
        self.input.clear();
        self.processing = true;
        
        debug!("Submitting user input: {}", input);
        
        // Handle special commands
        match input.as_str() {
            "/quit" | "/q" => {
                self.should_quit = true;
                return;
            }
            "/clear" => {
                self.session.clear();
                self.processing = false;
                return;
            }
            "/info" => {
                let info = self.session.agent_info();
                self.session.add_system_message(format!(
                    "Agent: {} v{} - {}",
                    info.name, info.version, info.description
                ));
                self.processing = false;
                return;
            }
            _ => {}
        }
        
        // Re-enable auto-scroll for new conversation
        self.auto_scroll_chat = true;
        
        // Process with session (this adds the user message immediately and
        // spawns a background task for the agent response)
        self.session.handle_user_input(input).await;
        // Keep `processing` true; it will be set to false when the
        // AgentResponse or AgentError event is received.
    }
    
    /// Draw the user interface
    fn ui(&mut self, f: &mut Frame) {
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints([
                Constraint::Percentage(60), // Chat area
                Constraint::Percentage(40), // Tools area
            ].as_ref())
            .split(f.size());
        
        // Left panel: Chat + Input + Status
        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),     // Chat area
                Constraint::Length(3),  // Input area
                Constraint::Length(1),  // Status line
            ].as_ref())
            .split(main_chunks[0]);
        
        // Chat display
        self.render_chat(f, left_chunks[0]);
        
        // Input area
        self.render_input(f, left_chunks[1]);
        
        // Status line
        self.render_status(f, left_chunks[2]);
        
        // Right panel: Tools
        self.render_tools(f, main_chunks[1]);
    }
    
    /// Render the chat messages
    fn render_chat(&mut self, f: &mut Frame, area: Rect) {
        // Prepare chat text
        let mut chat_lines = Vec::new();
        let available_width = area.width.saturating_sub(4) as usize; // Account for borders and padding
        
        for msg in self.session.messages() {
            match msg.role {
                grok_core::MessageRole::User => {
                    // User messages - simple styling with prefix
                    let content = format!("You: {}", msg.content);
                    let style = Style::default().fg(Color::Cyan);
                    
                    if content.len() <= available_width {
                        chat_lines.push(Line::from(Span::styled(content, style)));
                    } else {
                        // Wrap text
                        let words: Vec<&str> = content.split_whitespace().collect();
                        let mut current_line = String::new();
                        
                        for word in words {
                            if current_line.is_empty() {
                                current_line = word.to_string();
                            } else if current_line.len() + word.len() + 1 <= available_width {
                                current_line.push(' ');
                                current_line.push_str(word);
                            } else {
                                chat_lines.push(Line::from(Span::styled(current_line.clone(), style)));
                                current_line = word.to_string();
                            }
                        }
                        
                        if !current_line.is_empty() {
                            chat_lines.push(Line::from(Span::styled(current_line, style)));
                        }
                    }
                }
                grok_core::MessageRole::Agent => {
                    // Agent messages - parse markdown
                    // Add a subtle indicator that this is an agent response
                    chat_lines.push(Line::from(Span::styled(
                        "Agent:",
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    )));
                    
                    let markdown_lines = crate::markdown::parse_markdown(&msg.content);
                    let wrapped_lines = crate::markdown::wrap_markdown_lines(markdown_lines, available_width);
                    chat_lines.extend(wrapped_lines);
                }
                grok_core::MessageRole::System => {
                    // System messages - simple styling
                    let content = &msg.content;
                    let style = Style::default().fg(Color::Yellow);
                    
                    if content.len() <= available_width {
                        chat_lines.push(Line::from(Span::styled(content.clone(), style)));
                    } else {
                        // Wrap text
                        let words: Vec<&str> = content.split_whitespace().collect();
                        let mut current_line = String::new();
                        
                        for word in words {
                            if current_line.is_empty() {
                                current_line = word.to_string();
                            } else if current_line.len() + word.len() + 1 <= available_width {
                                current_line.push(' ');
                                current_line.push_str(word);
                            } else {
                                chat_lines.push(Line::from(Span::styled(current_line.clone(), style)));
                                current_line = word.to_string();
                            }
                        }
                        
                        if !current_line.is_empty() {
                            chat_lines.push(Line::from(Span::styled(current_line, style)));
                        }
                    }
                }
                grok_core::MessageRole::Error => {
                    // Error messages - simple styling
                    let content = &msg.content;
                    let style = Style::default().fg(Color::Red);
                    
                    if content.len() <= available_width {
                        chat_lines.push(Line::from(Span::styled(content.clone(), style)));
                    } else {
                        // Wrap text
                        let words: Vec<&str> = content.split_whitespace().collect();
                        let mut current_line = String::new();
                        
                        for word in words {
                            if current_line.is_empty() {
                                current_line = word.to_string();
                            } else if current_line.len() + word.len() + 1 <= available_width {
                                current_line.push(' ');
                                current_line.push_str(word);
                            } else {
                                chat_lines.push(Line::from(Span::styled(current_line.clone(), style)));
                                current_line = word.to_string();
                            }
                        }
                        
                        if !current_line.is_empty() {
                            chat_lines.push(Line::from(Span::styled(current_line, style)));
                        }
                    }
                }
            }
            
            // Add spacing between messages
            chat_lines.push(Line::from(""));
        }

        // Calculate scroll limits
        let content_height = chat_lines.len();
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
        let max_scroll = content_height.saturating_sub(visible_height);
        
        // Auto-scroll to bottom if enabled and there's new content
        let scroll_pos = if self.auto_scroll_chat {
            max_scroll
        } else {
            self.chat_scroll.min(max_scroll)
        };
        
        // Update the stored scroll position to prevent phantom scrolling
        self.chat_scroll = scroll_pos;

        // Slice visible content
        let visible_lines = if content_height > visible_height {
            chat_lines.into_iter().skip(scroll_pos).take(visible_height).collect()
        } else {
            chat_lines
        };

        let text = Text::from(visible_lines);
        
        let border_style = if self.focused_panel == 0 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        
        let title = if self.focused_panel == 0 {
            " Chat [FOCUSED] "
        } else {
            " Chat "
        };
        
        let chat = Paragraph::new(text)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title))
            .wrap(ratatui::widgets::Wrap { trim: false });
        
        f.render_widget(chat, area);

        // Render scrollbar if needed
        if content_height > visible_height {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
            // The scrollbar state should use the maximum scroll position as the total
            // When scroll_pos equals max_scroll, the thumb should be at the bottom
            let max_scroll = content_height.saturating_sub(visible_height);
            let mut scrollbar_state = ScrollbarState::new(max_scroll.max(1))
                .position(scroll_pos);
            f.render_stateful_widget(
                scrollbar,
                area.inner(&ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
                &mut scrollbar_state,
            );
        }
    }
    
    /// Render the input area
    fn render_input(&self, f: &mut Frame, area: Rect) {
        let input_text = if self.processing {
            "Processing...".to_string()
        } else {
            self.input.clone()
        };
        
        let border_style = if self.focused_panel == 0 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        
        let input = Paragraph::new(input_text)
            .style(if self.processing {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            })
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(" Input (Enter to send, Tab to switch focus, ↑↓ to scroll) "));
        
        f.render_widget(input, area);
    }
    
    /// Render the status line
    fn render_status(&self, f: &mut Frame, area: Rect) {
        let focus_indicator = match self.focused_panel {
            0 => format!("Chat focused{}", if self.auto_scroll_chat { " [Auto-scroll]" } else { "" }),
            1 => format!("Tools focused{}", if self.auto_scroll_tools { " [Auto-scroll]" } else { "" }),
            _ => "Unknown".to_string(),
        };
        
        let status_text = if self.processing {
            format!("● Processing... | {} | 'q' to quit, Tab to switch, ↑↓ to scroll, End to jump to bottom", focus_indicator)
        } else {
            format!("● Ready - Markdown supported! | {} | 'q' to quit, Tab to switch, ↑↓ to scroll, End to jump to bottom", focus_indicator)
        };
        
        let status = Paragraph::new(status_text)
            .style(if self.processing {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            });
        
        f.render_widget(status, area);
    }

    /// Render the tools panel
    fn render_tools(&mut self, f: &mut Frame, area: Rect) {
        let active_tools = self.session.active_tools();
        
        let border_style = if self.focused_panel == 1 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        
        let title = if self.focused_panel == 1 {
            " Tools [FOCUSED] "
        } else {
            " Tools "
        };
        
        if active_tools.is_empty() {
            let placeholder = Paragraph::new("No active tools\n\nPress Tab to switch focus\nUse ↑↓ to scroll when focused")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default()
                    .borders(Borders::ALL)
                    .border_style(border_style)
                    .title(title));
            f.render_widget(placeholder, area);
            return;
        }

        // Create a single scrollable text for all tools
        let mut all_lines = Vec::new();
        let available_width = area.width.saturating_sub(4) as usize; // Account for borders

        // Sort tools by start time (oldest first, so newest appear at bottom)
        let mut sorted_tools: Vec<_> = active_tools.iter().collect();
        sorted_tools.sort_by(|a, b| a.1.start_time.cmp(&b.1.start_time));
        
        for (tool_id, tool) in sorted_tools {
            // Tool header
            let status_text = match tool.status {
                ToolStatus::Running => "🔄 Running",
                ToolStatus::Completed => "✅ Completed", 
                ToolStatus::Failed => "❌ Failed",
            };
            
            let status_color = match tool.status {
                ToolStatus::Running => Color::Yellow,
                ToolStatus::Completed => Color::Green,
                ToolStatus::Failed => Color::Red,
            };

            // Add tool header lines
            let header1 = format!("Tool {}: {:?}", tool_id.chars().take(8).collect::<String>(), tool.tool);
            let header2 = format!("Summary: {}", tool.summary);
            let header3 = format!("Status: {}", status_text);
            
            all_lines.push(Line::from(Span::styled(header1, Style::default().fg(status_color).add_modifier(Modifier::BOLD))));
            all_lines.push(Line::from(Span::styled(header2, Style::default().fg(status_color))));
            all_lines.push(Line::from(Span::styled(header3, Style::default().fg(status_color))));
            all_lines.push(Line::from("─".repeat(available_width.min(60))));

            // Add tool content
            let content = match tool.status {
                ToolStatus::Running => {
                    if !tool.stdout.is_empty() || !tool.stderr.is_empty() {
                        format!("STDOUT:\n{}\n\nSTDERR:\n{}", tool.stdout, tool.stderr)
                    } else {
                        "Tool is running...".to_string()
                    }
                }
                ToolStatus::Completed | ToolStatus::Failed => {
                    let mut content = String::new();
                    
                    if !tool.stdout.is_empty() {
                        content.push_str(&format!("STDOUT:\n{}\n\n", tool.stdout));
                    }
                    
                    if !tool.stderr.is_empty() {
                        content.push_str(&format!("STDERR:\n{}\n\n", tool.stderr));
                    }
                    
                    if let Some(ref result) = tool.result {
                        content.push_str(&format!("RESULT:\n{}", serde_json::to_string_pretty(result).unwrap_or_else(|_| "Invalid JSON".to_string())));
                    }
                    
                    if content.is_empty() {
                        "No output".to_string()
                    } else {
                        content
                    }
                }
            };

            // Properly wrap content lines
            for line in content.lines() {
                if line.len() <= available_width {
                    all_lines.push(Line::from(line.to_string()));
                } else {
                    // Word wrap long lines
                    let words: Vec<&str> = line.split_whitespace().collect();
                    let mut current_line = String::new();
                    
                    for word in words {
                        if current_line.is_empty() {
                            current_line = word.to_string();
                        } else if current_line.len() + word.len() + 1 <= available_width {
                            current_line.push(' ');
                            current_line.push_str(word);
                        } else {
                            all_lines.push(Line::from(current_line.clone()));
                            current_line = word.to_string();
                        }
                    }
                    
                    if !current_line.is_empty() {
                        all_lines.push(Line::from(current_line));
                    }
                }
            }

            // Add spacing between tools
            all_lines.push(Line::from(""));
            all_lines.push(Line::from("═".repeat(available_width.min(60))));
            all_lines.push(Line::from(""));
        }

        // Calculate scroll for the entire tools panel
        let content_height = all_lines.len();
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
        let max_scroll = if content_height > visible_height {
            content_height.saturating_sub(visible_height)
        } else {
            0
        };
        
        // Auto-scroll to bottom if enabled and there's new content
        let scroll_pos = if self.auto_scroll_tools {
            max_scroll
        } else {
            self.tools_scroll.min(max_scroll)
        };
        
        // Update the stored scroll position to prevent phantom scrolling
        self.tools_scroll = scroll_pos;

        // Slice visible content
        let visible_lines = if content_height > visible_height {
            all_lines.into_iter().skip(scroll_pos).take(visible_height).collect()
        } else {
            all_lines
        };

        let text = Text::from(visible_lines);
        let tools_widget = Paragraph::new(text)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title))
            .wrap(ratatui::widgets::Wrap { trim: false });

        f.render_widget(tools_widget, area);

        // Render scrollbar if needed  
        if content_height > visible_height {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
            // The scrollbar state should use the maximum scroll position as the total
            // When scroll_pos equals max_scroll, the thumb should be at the bottom
            let mut scrollbar_state = ScrollbarState::new(max_scroll.max(1))
                .position(scroll_pos);
            f.render_stateful_widget(
                scrollbar,
                area.inner(&ratatui::layout::Margin { vertical: 1, horizontal: 0 }),
                &mut scrollbar_state,
            );
        }
    }
}

