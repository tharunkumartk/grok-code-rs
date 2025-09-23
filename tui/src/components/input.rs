use ratatui::{
    layout::Rect,
    style::{Color, Style, Modifier},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use crate::state::AppState;

/// Component for rendering the input panel
pub struct InputComponent;

impl InputComponent {
    /// Render the input area
    pub fn render(state: &mut AppState, f: &mut Frame, area: Rect) {
        if state.processing {
            let input = Paragraph::new("Processing...")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default()
                    .borders(Borders::ALL)
                    .title(" Input "));
            f.render_widget(input, area);
            return;
        }

        // Calculate available width for text (accounting for borders)
        let text_width = area.width.saturating_sub(2) as usize;
        let text_height = area.height.saturating_sub(2) as usize;

        // Split input into lines based on wrapping
        let mut lines = Vec::new();
        let mut current_line = String::new();
        let mut cursor_line = 0;
        let mut cursor_col = 0;
        let mut current_pos = 0;

        for (i, ch) in state.input.char_indices() {
            if ch == '\n' {
                // Handle explicit newlines
                lines.push(current_line);
                current_line = String::new();
                if current_pos <= state.input_cursor {
                    cursor_line += 1;
                    cursor_col = 0;
                }
            } else {
                if current_line.chars().count() >= text_width {
                    // Wrap line
                    lines.push(current_line);
                    current_line = String::new();
                    if current_pos < state.input_cursor {
                        cursor_line += 1;
                        cursor_col = 0;
                    }
                }

                current_line.push(ch);
                if current_pos < state.input_cursor {
                    cursor_col += 1;
                }
            }
            current_pos = i + ch.len_utf8();
        }

        if !current_line.is_empty() {
            lines.push(current_line);
        }

        // Handle case where cursor is at the end
        if state.input_cursor == state.input.len() {
            if let Some(last_line) = lines.last() {
                cursor_line = lines.len() - 1;
                cursor_col = last_line.chars().count();
            }
        }

        // Calculate scroll position
        let total_lines = lines.len();
        let max_scroll = total_lines.saturating_sub(text_height);
        let scroll_pos = state.input_scroll.min(max_scroll);

        // Get visible lines
        let visible_lines: Vec<String> = lines.into_iter()
            .skip(scroll_pos)
            .take(text_height)
            .collect();

        // Adjust cursor position for scrolling
        let visible_cursor_line = if cursor_line >= scroll_pos {
            cursor_line - scroll_pos
        } else {
            0
        };

        // Create the display text
        let display_text = visible_lines.join("\n");

        let border_style = if state.focused_panel == 0 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let title = if state.focused_panel == 0 {
            " Input [FOCUSED] (Enter to send, Tab to switch focus) "
        } else {
            " Input "
        };

        let input_widget = Paragraph::new(display_text)
            .block(Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title))
            .wrap(ratatui::widgets::Wrap { trim: false });

        f.render_widget(input_widget, area);

        // Render cursor if focused and visible
        if state.focused_panel == 0 && state.cursor_visible && visible_cursor_line < text_height {
            let cursor_x = area.x + 1 + cursor_col as u16;
            let cursor_y = area.y + 1 + visible_cursor_line as u16;

            // Make sure cursor is within bounds
            if cursor_x < area.x + area.width - 1 && cursor_y < area.y + area.height - 1 {
                f.set_cursor(cursor_x, cursor_y);
            }
        }

        // Render scrollbar if needed
        if total_lines > text_height {
            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));
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
