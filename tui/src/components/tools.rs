use ratatui::{
    layout::Rect,
    style::{Color, Style, Modifier},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    Frame,
};
use grok_core::ToolStatus;
use crate::state::AppState;

/// Component for rendering the tools panel
pub struct ToolsComponent;

impl ToolsComponent {
    /// Render the tools panel
    pub fn render(state: &mut AppState, f: &mut Frame, area: Rect) {
        let active_tools = state.session.active_tools();
        
        let border_style = if state.focused_panel == 2 {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let title = if state.focused_panel == 2 {
            " Tools [FOCUSED] "
        } else {
            " Tools "
        };
        
        if active_tools.is_empty() {
            let placeholder = Paragraph::new("No active tools\n\nPress Tab to switch focus\nUse ↑↓ or scroll wheel to scroll when focused")
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

        // If width is too small, don't wrap to avoid issues
        let should_wrap = available_width >= 10;

        // Sort tools by start time (oldest first, so newest appear at bottom)
        let mut sorted_tools: Vec<_> = active_tools.iter().collect();
        sorted_tools.sort_by(|a, b| a.1.start_time.cmp(&b.1.start_time));
        
        for (_tool_id, tool) in sorted_tools {
            Self::render_tool(&mut all_lines, tool, available_width, should_wrap);

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
        let scroll_pos = if state.auto_scroll_tools {
            max_scroll
        } else {
            state.tools_scroll.min(max_scroll)
        };
        
        // Update the stored scroll position to prevent phantom scrolling
        state.tools_scroll = scroll_pos;

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

    fn render_tool(all_lines: &mut Vec<Line>, tool: &grok_core::ActiveTool, available_width: usize, should_wrap: bool) {
        // Tool header
        let status_icon = match tool.status {
            ToolStatus::Running => "🔄",
            ToolStatus::Completed => "✅", 
            ToolStatus::Failed => "❌",
        };
        
        let status_color = match tool.status {
            ToolStatus::Running => Color::Yellow,
            ToolStatus::Completed => Color::Green,
            ToolStatus::Failed => Color::Red,
        };

        // Create a cleaner header format
        let tool_name = format!("{:?}", tool.tool);
        let header = Self::format_tool_header(&tool_name, &tool.summary, status_icon);
        
        all_lines.push(Line::from(Span::styled(header, Style::default().fg(status_color).add_modifier(Modifier::BOLD))));
        all_lines.push(Line::from("─".repeat(available_width.min(60))));

        // Add tool parameters if available and relevant
        if let Some(ref args) = tool.args {
            Self::render_tool_parameters(all_lines, &tool.tool, args);
        }

        // Add tool content
        let content = Self::format_tool_content(tool);

        // Properly wrap content lines
        for line in content.lines() {
            Self::add_wrapped_line(all_lines, line, available_width, should_wrap);
        }
    }

    fn format_tool_header(tool_name: &str, summary: &str, status_icon: &str) -> String {
        if summary.starts_with(&format!("{} file:", tool_name.replace("Fs", "").to_lowercase())) {
            // For file operations like "Reading file: path", extract just the filename
            let filename = summary.split(": ").nth(1).unwrap_or(summary);
            let basename = std::path::Path::new(filename).file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(filename);
            format!("{} {} {}", status_icon, tool_name, basename)
        } else if summary.starts_with("Searching for:") {
            // For search operations
            let query = summary.split(": ").nth(1).unwrap_or(summary);
            format!("{} {} \"{}\"", status_icon, tool_name, query)
        } else if summary.starts_with("Executing:") {
            // For shell commands
            let command = summary.split(": ").nth(1).unwrap_or(summary);
            format!("{} {} {}", status_icon, tool_name, command)
        } else {
            // Fallback to original summary
            format!("{} {}", status_icon, summary)
        }
    }

    fn render_tool_parameters(all_lines: &mut Vec<Line>, tool: &grok_core::ToolName, args: &serde_json::Value) {
        match tool {
            grok_core::ToolName::FsSearch => {
                if let Ok(search_args) = serde_json::from_value::<grok_core::tools::FsSearchArgs>(args.clone()) {
                    all_lines.push(Line::from(Span::styled("Parameters:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
                    all_lines.push(Line::from(format!("  Query: {}", search_args.query)));
                    if let Some(ref globs) = search_args.globs {
                        all_lines.push(Line::from(format!("  Globs: {}", globs.join(", "))));
                    }
                    if let Some(max_results) = search_args.max_results {
                        all_lines.push(Line::from(format!("  Max results: {}", max_results)));
                    }
                    if search_args.regex {
                        all_lines.push(Line::from("  Mode: Regex"));
                    }
                    if search_args.case_insensitive {
                        all_lines.push(Line::from("  Case insensitive: true"));
                    }
                    if search_args.multiline {
                        all_lines.push(Line::from("  Multiline: true"));
                    }
                    all_lines.push(Line::from(""));
                }
            }
            grok_core::ToolName::FsRead => {
                if let Ok(read_args) = serde_json::from_value::<grok_core::tools::FsReadArgs>(args.clone()) {
                    all_lines.push(Line::from(Span::styled("Parameters:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
                    all_lines.push(Line::from(format!("  Path: {}", read_args.path)));
                    if let Some(ref range) = read_args.range {
                        all_lines.push(Line::from(format!("  Range: {}..{}", range.start, range.end)));
                    }
                    if let Some(ref encoding) = read_args.encoding {
                        all_lines.push(Line::from(format!("  Encoding: {}", encoding)));
                    }
                    all_lines.push(Line::from(""));
                }
            }
            grok_core::ToolName::FsWrite => {
                if let Ok(write_args) = serde_json::from_value::<grok_core::tools::FsWriteArgs>(args.clone()) {
                    all_lines.push(Line::from(Span::styled("Parameters:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
                    all_lines.push(Line::from(format!("  Path: {}", write_args.path)));
                    all_lines.push(Line::from(format!("  Size: {} bytes", write_args.contents.len())));
                    if write_args.create_if_missing {
                        all_lines.push(Line::from("  Create if missing: true"));
                    }
                    if write_args.overwrite {
                        all_lines.push(Line::from("  Overwrite: true"));
                    }
                    all_lines.push(Line::from(""));
                }
            }
            grok_core::ToolName::ShellExec => {
                if let Ok(shell_args) = serde_json::from_value::<grok_core::tools::ShellExecArgs>(args.clone()) {
                    all_lines.push(Line::from(Span::styled("Parameters:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
                    all_lines.push(Line::from(format!("  Command: {}", shell_args.command.join(" "))));
                    if let Some(ref cwd) = shell_args.cwd {
                        all_lines.push(Line::from(format!("  Working directory: {}", cwd)));
                    }
                    if let Some(timeout) = shell_args.timeout_ms {
                        all_lines.push(Line::from(format!("  Timeout: {}ms", timeout)));
                    }
                    if let Some(escalated) = shell_args.with_escalated_permissions {
                        if escalated {
                            all_lines.push(Line::from("  Escalated permissions: true"));
                        }
                    }
                    all_lines.push(Line::from(""));
                }
            }
            grok_core::ToolName::FsApplyPatch => {
                if let Ok(patch_args) = serde_json::from_value::<grok_core::tools::FsApplyPatchArgs>(args.clone()) {
                    all_lines.push(Line::from(Span::styled("Parameters:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
                    all_lines.push(Line::from(format!("  Diff size: {} characters", patch_args.unified_diff.len())));
                    if patch_args.dry_run {
                        all_lines.push(Line::from("  Mode: Dry run"));
                    }
                    all_lines.push(Line::from(""));
                }
            }
            grok_core::ToolName::FsFind => {
                if let Ok(find_args) = serde_json::from_value::<grok_core::tools::FsFindArgs>(args.clone()) {
                    all_lines.push(Line::from(Span::styled("Parameters:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
                    all_lines.push(Line::from(format!("  Pattern: {}", find_args.pattern)));
                    if let Some(ref base_path) = find_args.base_path {
                        all_lines.push(Line::from(format!("  Base path: {}", base_path)));
                    }
                    if let Some(fuzzy) = find_args.fuzzy {
                        all_lines.push(Line::from(format!("  Fuzzy matching: {}", fuzzy)));
                    }
                    if let Some(max_results) = find_args.max_results {
                        all_lines.push(Line::from(format!("  Max results: {}", max_results)));
                    }
                    all_lines.push(Line::from(""));
                }
            }
            grok_core::ToolName::CodeSymbols => {
                if let Ok(symbols_args) = serde_json::from_value::<grok_core::tools::CodeSymbolsArgs>(args.clone()) {
                    all_lines.push(Line::from(Span::styled("Parameters:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))));
                    all_lines.push(Line::from(format!("  File: {}", symbols_args.path)));
                    if let Some(ref language) = symbols_args.language {
                        all_lines.push(Line::from(format!("  Language: {}", language)));
                    }
                    if let Some(ref symbol_types) = symbols_args.symbol_types {
                        all_lines.push(Line::from(format!("  Symbol types: {}", symbol_types.join(", "))));
                    }
                    all_lines.push(Line::from(""));
                }
            }
        }
    }

    fn format_tool_content(tool: &grok_core::ActiveTool) -> String {
        match tool.status {
            ToolStatus::Running => {
                let mut content = String::new();
                if !tool.stdout.is_empty() {
                    content.push_str(&format!("STDOUT:\n{}", tool.stdout));
                }
                if !tool.stderr.is_empty() {
                    if !content.is_empty() { content.push_str("\n\n"); }
                    let concise_error = Self::make_error_concise(&tool.stderr);
                    content.push_str(&format!("ERROR:\n{}", concise_error));
                }
                if content.is_empty() {
                    "Tool is running...".to_string()
                } else {
                    content
                }
            }
            ToolStatus::Completed | ToolStatus::Failed => {
                let mut content = String::new();
                
                // For failed tools, show error information prominently first
                if tool.status == ToolStatus::Failed {
                    if !tool.stderr.is_empty() {
                        let concise_error = Self::make_error_concise(&tool.stderr);
                        content.push_str(&format!("❌ FAILED: {}", concise_error));
                    } else {
                        content.push_str("❌ FAILED: Tool failed with no error details");
                    }
                }
                
                // For both completed and failed tools, show structured result if available
                if let Some(ref result) = tool.result {
                    if !content.is_empty() { content.push_str("\n\n"); }
                    content.push_str(&Self::format_tool_result(&tool.tool, result));
                }
                
                // Add stdout if it exists and is meaningful (for completed tools or if no result)
                if !tool.stdout.is_empty() && (tool.status == ToolStatus::Completed || tool.result.is_none()) {
                    if !content.is_empty() { content.push_str("\n\n"); }
                    content.push_str(&format!("STDOUT:\n{}", tool.stdout));
                }
                
                // For completed tools, show stderr only if we haven't already shown it for failures
                if !tool.stderr.is_empty() && tool.status == ToolStatus::Completed {
                    if !content.is_empty() { content.push_str("\n\n"); }
                    let concise_error = Self::make_error_concise(&tool.stderr);
                    content.push_str(&format!("ERROR:\n{}", concise_error));
                }
                
                if content.is_empty() {
                    "No output".to_string()
                } else {
                    content
                }
            }
        }
    }

    fn format_tool_result(tool: &grok_core::ToolName, result: &serde_json::Value) -> String {
        match tool {
            grok_core::ToolName::FsRead => {
                if let Ok(fs_result) = serde_json::from_value::<serde_json::Value>(result.clone()) {
                    if let Some(contents) = fs_result.get("contents").and_then(|c| c.as_str()) {
                        // For file reads, show the actual file contents directly but limit display length
                        const MAX_DISPLAY_LENGTH: usize = 5000; // Limit to 5000 characters for UI display
                        let mut display_contents = if contents.len() > MAX_DISPLAY_LENGTH {
                            format!("{}\n\n[Content truncated for display - showing first {} of {} characters]", &contents[..MAX_DISPLAY_LENGTH], MAX_DISPLAY_LENGTH, contents.len())
                        } else {
                            contents.to_string()
                        };

                        if let Some(truncated) = fs_result.get("truncated").and_then(|t| t.as_bool()) {
                            if truncated {
                                display_contents.push_str("\n\n[File was truncated during reading...]");
                            }
                        }
                        display_contents
                    } else {
                        // Fallback to pretty JSON
                        serde_json::to_string_pretty(result).unwrap_or_else(|_| "Invalid JSON".to_string())
                    }
                } else {
                    serde_json::to_string_pretty(result).unwrap_or_else(|_| "Invalid JSON".to_string())
                }
            }
            grok_core::ToolName::FsSearch => {
                if let Ok(search_result) = serde_json::from_value::<serde_json::Value>(result.clone()) {
                    if let Some(matches) = search_result.get("matches").and_then(|m| m.as_array()) {
                        if matches.is_empty() {
                            "No matches found".to_string()
                        } else {
                            let mut content = String::new();
                            for (i, match_obj) in matches.iter().enumerate() {
                                if i > 0 { content.push_str("\n\n"); }
                                if let Some(path) = match_obj.get("path").and_then(|p| p.as_str()) {
                                    content.push_str(&format!("📁 {}\n", path));
                                }
                                if let Some(lines) = match_obj.get("lines").and_then(|l| l.as_array()) {
                                    for line in lines {
                                        if let (Some(ln), Some(text)) = (
                                            line.get("ln").and_then(|l| l.as_u64()),
                                            line.get("text").and_then(|t| t.as_str())
                                        ) {
                                            content.push_str(&format!("  {}| {}\n", ln, text));
                                        }
                                    }
                                }
                            }
                            content
                        }
                    } else {
                        serde_json::to_string_pretty(result).unwrap_or_else(|_| "Invalid JSON".to_string())
                    }
                } else {
                    serde_json::to_string_pretty(result).unwrap_or_else(|_| "Invalid JSON".to_string())
                }
            }
            _ => {
                // Handle other tool types with their specific result formatting
                // This is a simplified version - you'd want to implement specific formatting for each tool
                serde_json::to_string_pretty(result).unwrap_or_else(|_| "Invalid JSON".to_string())
            }
        }
    }

    fn add_wrapped_line(all_lines: &mut Vec<Line>, line: &str, available_width: usize, should_wrap: bool) {
        if line.len() <= available_width && should_wrap {
            all_lines.push(Line::from(line.to_string()));
        } else if should_wrap {
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
        } else {
            // Don't wrap - just add as single line
            all_lines.push(Line::from(line.to_string()));
        }
    }

    /// Make error messages more concise for display
    fn make_error_concise(error_text: &str) -> String {
        // Take only the first few lines of stderr to avoid overwhelming the UI
        const MAX_ERROR_LINES: usize = 5;
        const MAX_LINE_LENGTH: usize = 120;
        
        let lines: Vec<&str> = error_text.lines().collect();
        let mut result_lines = Vec::new();
        
        for (i, line) in lines.iter().enumerate() {
            if i >= MAX_ERROR_LINES {
                result_lines.push(format!("... ({} more lines)", lines.len() - i));
                break;
            }
            
            // Truncate very long lines
            if line.len() > MAX_LINE_LENGTH {
                result_lines.push(format!("{}...", &line[..MAX_LINE_LENGTH]));
            } else {
                result_lines.push(line.to_string());
            }
        }
        
        result_lines.join("\n")
    }
}
