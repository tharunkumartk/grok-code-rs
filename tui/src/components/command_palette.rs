use ratatui::{
    layout::Rect,
    style::{Color, Style, Modifier},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use crate::state::{AppState, Command};

/// Component for rendering the command palette overlay
pub struct CommandPaletteComponent;

impl CommandPaletteComponent {
    /// Render the command palette overlay
    pub fn render(state: &mut AppState, f: &mut Frame) {
        let area = f.size();
        
        // Calculate popup size (centered, 60% width, 50% height)
        let popup_width = area.width * 60 / 100;
        let popup_height = area.height * 50 / 100;
        let popup_x = (area.width - popup_width) / 2;
        let popup_y = (area.height - popup_height) / 2;
        
        let popup_area = Rect {
            x: popup_x,
            y: popup_y,
            width: popup_width,
            height: popup_height,
        };

        // Clear the background
        f.render_widget(Clear, popup_area);

        // Get filtered commands
        let filtered_commands = Self::get_filtered_commands(state);

        // Prepare command list text
        let mut lines = vec![
            Line::from(Span::styled(
                format!("Command Palette {}", if state.command_palette_filter.is_empty() { 
                    "(type to filter)".to_string() 
                } else { 
                    format!("(filter: {})", state.command_palette_filter) 
                }),
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        if filtered_commands.is_empty() {
            lines.push(Line::from(Span::styled(
                "No matching commands found",
                Style::default().fg(Color::Red),
            )));
        } else {
            for (i, cmd) in filtered_commands.iter().enumerate() {
                let is_selected = i == state.command_palette_selected;
                let style = if is_selected {
                    Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                // Command name and syntax
                lines.push(Line::from(vec![
                    Span::styled(
                        if is_selected { "► " } else { "  " },
                        style,
                    ),
                    Span::styled(
                        cmd.name.clone(),
                        style.fg(if is_selected { Color::Yellow } else { Color::Green }),
                    ),
                ]));

                // Command description
                lines.push(Line::from(vec![
                    Span::styled("    ", style),
                    Span::styled(
                        cmd.description.clone(),
                        style.fg(if is_selected { Color::White } else { Color::Gray }),
                    ),
                ]));

                if i < filtered_commands.len() - 1 {
                    lines.push(Line::from(""));
                }
            }
        }

        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "↑↓ Navigate • Enter Select • Esc Close",
            Style::default().fg(Color::DarkGray),
        )));

        let text = Text::from(lines);
        let popup = Paragraph::new(text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                    .title(" Commands ")
                    .title_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            )
            .wrap(ratatui::widgets::Wrap { trim: false });

        f.render_widget(popup, popup_area);
    }

    /// Get filtered commands based on current filter
    fn get_filtered_commands(state: &AppState) -> Vec<&Command> {
        state
            .available_commands
            .iter()
            .filter(|cmd| {
                if state.command_palette_filter.is_empty() {
                    true
                } else {
                    cmd.name
                        .to_lowercase()
                        .contains(&state.command_palette_filter.to_lowercase())
                        || cmd
                            .description
                            .to_lowercase()
                            .contains(&state.command_palette_filter.to_lowercase())
                }
            })
            .collect()
    }
}
