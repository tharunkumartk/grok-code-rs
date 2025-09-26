use ratatui::{
    layout::Rect,
    style::{Color, Style, Modifier},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use crate::state::AppState;
use serde_json::Value;

/// Component for rendering the chat panel
pub struct ChatComponent;

impl ChatComponent {
    /// Render the chat messages
    pub fn render(state: &mut AppState, f: &mut Frame, area: Rect) {
        // Prepare chat text
        let mut chat_lines = Vec::new();
        let available_width = area.width.saturating_sub(4) as usize; // Account for borders and padding

        // If width is too small, don't wrap to avoid issues
        let should_wrap = available_width >= 10;
        
        // NOTE: Include tool messages in the chat render so they are not hidden
        for msg in state.session.messages() {
            match msg.role {
                grok_core::MessageRole::User => {
                    Self::render_user_message(&mut chat_lines, &msg.content, available_width, should_wrap);
                }
                grok_core::MessageRole::Agent => {
                    Self::render_agent_message(&mut chat_lines, &msg.content, available_width);
                }
                grok_core::MessageRole::System => {
                    Self::render_system_message(&mut chat_lines, &msg.content, available_width, should_wrap);
                }
                grok_core::MessageRole::Error => {
                    Self::render_error_message(&mut chat_lines, &msg.content, available_width, should_wrap);
                }
                grok_core::MessageRole::Tool => {
                    Self::render_tool_message(&mut chat_lines, msg.tool_info.as_ref(), available_width, should_wrap);
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
        let scroll_pos = if state.auto_scroll_chat {
            max_scroll
        } else {
            state.chat_scroll.min(max_scroll)
        };
        
        // Update the stored scroll position to prevent phantom scrolling
        state.chat_scroll = scroll_pos;

        // Slice visible content
        let visible_lines = if content_height > visible_height {
            chat_lines.into_iter().skip(scroll_pos).take(visible_height).collect()
        } else {
            chat_lines
        };

        let text = Text::from(visible_lines);
        
        let border_style = if state.focused_panel == 1 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let title = if state.focused_panel == 1 {
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

    fn render_user_message(chat_lines: &mut Vec<Line>, content: &str, available_width: usize, should_wrap: bool) {
        // User messages - simple styling with prefix
        let content = format!("You: {}", content);
        let style = Style::default().fg(Color::Cyan);

        Self::add_wrapped_text(chat_lines, &content, style, available_width, should_wrap);
    }

    fn render_agent_message(chat_lines: &mut Vec<Line>, content: &str, available_width: usize) {
        // Agent messages - parse markdown
        // Add a subtle indicator that this is an agent response
        chat_lines.push(Line::from(Span::styled(
            "Agent:",
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        )));
        
        let markdown_lines = crate::markdown::parse_markdown(content);
        let wrapped_lines = crate::markdown::wrap_markdown_lines(markdown_lines, available_width);
        chat_lines.extend(wrapped_lines);
    }

    fn render_system_message(chat_lines: &mut Vec<Line>, content: &str, available_width: usize, should_wrap: bool) {
        // System messages - simple styling
        let style = Style::default().fg(Color::Yellow);
        Self::add_wrapped_text(chat_lines, content, style, available_width, should_wrap);
    }

    fn render_error_message(chat_lines: &mut Vec<Line>, content: &str, available_width: usize, should_wrap: bool) {
        // Error messages - simple styling
        let style = Style::default().fg(Color::Red);
        Self::add_wrapped_text(chat_lines, content, style, available_width, should_wrap);
    }

    fn render_tool_message(
        chat_lines: &mut Vec<Line>,
        tool_info: Option<&grok_core::ToolMessageInfo>,
        available_width: usize,
        should_wrap: bool,
    ) {
        let style = Style::default().fg(Color::Magenta);

        let text = match tool_info {
            Some(info) => {
                let tool_name = format!("{:?}", info.tool);
                let params = match &info.args {
                    Some(v) => Self::format_params(v),
                    None => "none".to_string(),
                };
                format!("Agent ran {} with parameters {}", tool_name, params)
            }
            None => {
                // Fallback (shouldn't normally happen)
                "Agent ran a tool".to_string()
            }
        };

        Self::add_wrapped_text(chat_lines, &text, style, available_width, should_wrap);
    }

    fn format_params(v: &Value) -> String {
        // Prefer a compact k=v list for objects; otherwise JSON string
        if let Some(map) = v.as_object() {
            let mut parts: Vec<String> = Vec::new();
            for (k, val) in map.iter() {
                let s = if val.is_string() {
                    format!("{}=\"{}\"", k, val.as_str().unwrap_or(""))
                } else if val.is_number() || val.is_boolean() {
                    format!("{}={}", k, val)
                } else {
                    // For arrays/objects, include compact JSON
                    format!("{}={}", k, val)
                };
                parts.push(s);
            }
            if parts.is_empty() { "{}".to_string() } else { parts.join(", ") }
        } else {
            v.to_string()
        }
    }

    fn add_wrapped_text(chat_lines: &mut Vec<Line>, content: &str, style: Style, available_width: usize, should_wrap: bool) {
        if content.len() <= available_width && should_wrap {
            chat_lines.push(Line::from(Span::styled(content.to_string(), style)));
        } else if should_wrap {
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
        } else {
            // Don't wrap - just add as single line
            chat_lines.push(Line::from(Span::styled(content.to_string(), style)));
        }
    }
}
