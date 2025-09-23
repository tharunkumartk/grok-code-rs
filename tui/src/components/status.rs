use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::Paragraph,
    Frame,
};
use crate::state::AppState;

/// Component for rendering the status line
pub struct StatusComponent;

impl StatusComponent {
    /// Render the status line
    pub fn render(state: &AppState, f: &mut Frame, area: Rect) {
        let focus_indicator = match state.focused_panel {
            0 => "Input focused".to_string(),
            1 => format!("Chat focused{}", if state.auto_scroll_chat { " [Auto-scroll]" } else { "" }),
            2 => format!("Tools focused{}", if state.auto_scroll_tools { " [Auto-scroll]" } else { "" }),
            _ => "Unknown".to_string(),
        };
        
        let status_text = if state.processing {
            format!("● Processing... | {} | 'q' to quit, Tab to switch, '/' for commands, ↑↓/scroll wheel to scroll, End to jump to bottom", focus_indicator)
        } else {
            "Ready - Grok Code CLI | / for commands | Tab to switch".to_string()
        };
        
        let status = Paragraph::new(status_text)
            .style(if state.processing {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::Green)
            });
        
        f.render_widget(status, area);
    }
}
