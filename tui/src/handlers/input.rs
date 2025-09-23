use crossterm::event::{Event, KeyCode, KeyEventKind, MouseEvent, MouseEventKind};
use crate::state::AppState;

/// Handles input events for the application
pub struct InputHandler;

impl InputHandler {
    /// Handle input events (keyboard and mouse)
    pub async fn handle_event(state: &mut AppState, event: crossterm::event::Event) {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                Self::handle_key_event(state, key.code, key.modifiers).await;
            }
            Event::Mouse(mouse_event) => {
                Self::handle_mouse_event(state, mouse_event);
            }
            _ => {}
        }
    }

    async fn handle_key_event(
        state: &mut AppState,
        key_code: KeyCode,
        modifiers: crossterm::event::KeyModifiers,
    ) {
        use crossterm::event::KeyModifiers;

        match key_code {
            KeyCode::Char('q') if !state.processing => {
                state.should_quit = true;
            }
            KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                state.should_quit = true;
            }
            KeyCode::Tab => {
                // Switch between panels (chat input, chat history, tools)
                state.focused_panel = (state.focused_panel + 1) % 3;
            }
            KeyCode::Up => {
                Self::handle_up_key(state);
            }
            KeyCode::Down => {
                Self::handle_down_key(state);
            }
            KeyCode::PageUp => {
                Self::handle_page_up(state);
            }
            KeyCode::PageDown => {
                Self::handle_page_down(state);
            }
            KeyCode::Enter if state.focused_panel == 0 => {
                if state.command_palette_open {
                    Self::execute_selected_command(state).await;
                } else {
                    Self::submit_input(state).await;
                }
            }
            KeyCode::Char('/') if state.focused_panel == 0 && state.input.is_empty() && !state.command_palette_open => {
                // Open command palette when typing '/' at the beginning of empty input
                state.command_palette_open = true;
                state.command_palette_selected = 0;
                state.command_palette_filter.clear();
            }
            KeyCode::Char(c) if state.focused_panel == 0 => {
                if state.command_palette_open {
                    Self::handle_command_palette_char(state, c);
                } else {
                    Self::insert_char(state, c);
                }
            }
            KeyCode::Backspace if state.focused_panel == 0 => {
                if state.command_palette_open {
                    if !state.command_palette_filter.is_empty() {
                        state.command_palette_filter.pop();
                        // Reset selection when filter changes
                        state.command_palette_selected = 0;
                    } else {
                        // Close command palette if filter is empty and backspace is pressed
                        state.command_palette_open = false;
                    }
                } else {
                    Self::delete_char(state);
                }
            }
            KeyCode::Left if state.focused_panel == 0 => {
                Self::move_cursor_left(state);
            }
            KeyCode::Right if state.focused_panel == 0 => {
                Self::move_cursor_right(state);
            }
            KeyCode::Home if state.focused_panel == 0 => {
                state.input_cursor = 0;
            }
            KeyCode::End if state.focused_panel == 0 => {
                state.input_cursor = state.input.len();
            }
            KeyCode::Esc => {
                if state.command_palette_open {
                    // Close command palette
                    state.command_palette_open = false;
                    state.command_palette_filter.clear();
                    state.command_palette_selected = 0;
                } else {
                    state.input.clear();
                    state.focused_panel = 0; // Return focus to chat
                }
            }
            KeyCode::End => {
                // Jump to bottom and re-enable auto-scroll for focused panel
                match state.focused_panel {
                    0 => {
                        // Input area - go to end of input
                        state.input_cursor = state.input.len();
                    }
                    1 => {
                        // Chat history
                        state.auto_scroll_chat = true;
                    }
                    2 => {
                        // Tools
                        state.auto_scroll_tools = true;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn handle_mouse_event(state: &mut AppState, mouse_event: MouseEvent) {
        match mouse_event.kind {
            MouseEventKind::ScrollUp => {
                // Scroll up in focused panel
                match state.focused_panel {
                    0 => {
                        // Input area - scroll up
                        if state.input_scroll > 0 {
                            state.input_scroll = state.input_scroll.saturating_sub(1);
                        }
                    }
                    1 => {
                        // Chat history
                        if state.chat_scroll > 0 {
                            state.chat_scroll = state.chat_scroll.saturating_sub(3); // Scroll 3 lines at a time
                            // Disable auto-scroll when user manually scrolls
                            state.auto_scroll_chat = false;
                        }
                    }
                    2 => {
                        // Tools
                        if state.tools_scroll > 0 {
                            state.tools_scroll = state.tools_scroll.saturating_sub(3); // Scroll 3 lines at a time
                            // Disable auto-scroll when user manually scrolls
                            state.auto_scroll_tools = false;
                        }
                    }
                    _ => {}
                }
            }
            MouseEventKind::ScrollDown => {
                // Scroll down in focused panel
                match state.focused_panel {
                    0 => {
                        // Input area - scroll down
                        state.input_scroll = state.input_scroll.saturating_add(1);
                    }
                    1 => {
                        // Chat history
                        state.chat_scroll = state.chat_scroll.saturating_add(3); // Scroll 3 lines at a time
                        // Disable auto-scroll when user manually scrolls
                        state.auto_scroll_chat = false;
                    }
                    2 => {
                        // Tools
                        state.tools_scroll = state.tools_scroll.saturating_add(3); // Scroll 3 lines at a time
                        // Disable auto-scroll when user manually scrolls
                        state.auto_scroll_tools = false;
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    fn handle_up_key(state: &mut AppState) {
        if state.command_palette_open && state.focused_panel == 0 {
            // Navigate command palette
            if state.command_palette_selected > 0 {
                state.command_palette_selected -= 1;
            }
        } else {
            // Scroll up in focused panel
            match state.focused_panel {
                0 => {
                    // Input area - move cursor up in multi-line input
                    Self::move_cursor_up(state);
                }
                1 => {
                    // Chat history
                    if state.chat_scroll > 0 {
                        state.chat_scroll = state.chat_scroll.saturating_sub(1);
                        // Disable auto-scroll when user manually scrolls
                        state.auto_scroll_chat = false;
                    }
                }
                2 => {
                    // Tools
                    if state.tools_scroll > 0 {
                        state.tools_scroll = state.tools_scroll.saturating_sub(1);
                        // Disable auto-scroll when user manually scrolls
                        state.auto_scroll_tools = false;
                    }
                }
                _ => {}
            }
        }
    }

    fn handle_down_key(state: &mut AppState) {
        if state.command_palette_open && state.focused_panel == 0 {
            // Navigate command palette
            let filtered_commands = Self::get_filtered_commands(state);
            if state.command_palette_selected < filtered_commands.len().saturating_sub(1) {
                state.command_palette_selected += 1;
            }
        } else {
            // Scroll down in focused panel
            match state.focused_panel {
                0 => {
                    // Input area - move cursor down in multi-line input
                    Self::move_cursor_down(state);
                }
                1 => {
                    // Chat history
                    state.chat_scroll = state.chat_scroll.saturating_add(1);
                    // Disable auto-scroll when user manually scrolls
                    state.auto_scroll_chat = false;
                }
                2 => {
                    // Tools
                    state.tools_scroll = state.tools_scroll.saturating_add(1);
                    // Disable auto-scroll when user manually scrolls
                    state.auto_scroll_tools = false;
                }
                _ => {}
            }
        }
    }

    fn handle_page_up(state: &mut AppState) {
        // Page up in focused panel
        match state.focused_panel {
            0 => {
                // Input area - scroll up
                if state.input_scroll > 0 {
                    state.input_scroll = state.input_scroll.saturating_sub(5);
                }
            }
            1 => {
                // Chat history
                if state.chat_scroll >= 10 {
                    state.chat_scroll = state.chat_scroll.saturating_sub(10);
                } else {
                    state.chat_scroll = 0;
                }
                // Disable auto-scroll when user manually scrolls
                state.auto_scroll_chat = false;
            }
            2 => {
                // Tools
                if state.tools_scroll >= 10 {
                    state.tools_scroll = state.tools_scroll.saturating_sub(10);
                } else {
                    state.tools_scroll = 0;
                }
                // Disable auto-scroll when user manually scrolls
                state.auto_scroll_tools = false;
            }
            _ => {}
        }
    }

    fn handle_page_down(state: &mut AppState) {
        // Page down in focused panel
        match state.focused_panel {
            0 => {
                // Input area - scroll down
                state.input_scroll = state.input_scroll.saturating_add(5);
            }
            1 => {
                // Chat history
                state.chat_scroll = state.chat_scroll.saturating_add(10);
                // Disable auto-scroll when user manually scrolls
                state.auto_scroll_chat = false;
            }
            2 => {
                // Tools
                state.tools_scroll = state.tools_scroll.saturating_add(10);
                // Disable auto-scroll when user manually scrolls
                state.auto_scroll_tools = false;
            }
            _ => {}
        }
    }

    /// Submit the current input to the agent
    async fn submit_input(state: &mut AppState) {
        if state.input.trim().is_empty() || state.processing {
            return;
        }

        let input = state.input.trim().to_string();
        state.input.clear();
        state.input_cursor = 0;
        state.input_scroll = 0;
        state.processing = true;

        // Handle special commands
        match input.as_str() {
            "/quit" | "/q" => {
                state.should_quit = true;
                return;
            }
            "/clear" => {
                state.session.clear();
                state.processing = false;
                return;
            }
            "/info" => {
                let info = state.session.agent_info();
                state.session.add_system_message(format!(
                    "Agent: {} v{} - {}",
                    info.name, info.version, info.description
                ));
                state.processing = false;
                return;
            }
            "/context" => {
                if let Some(usage) = &state.current_token_usage {
                    state.session.add_system_message(format!(
                        "Token Usage:\n• Input tokens: {}\n• Output tokens: {}\n• Total tokens: {}",
                        usage.input_tokens, usage.output_tokens, usage.total_tokens
                    ));
                } else {
                    state.session.add_system_message("No token usage information available yet.".to_string());
                }
                state.processing = false;
                return;
            }
            "/thinking" => {
                let current_status = std::env::var("GROK_ENABLE_INTERLEAVED_THINKING")
                    .map(|v| v.to_lowercase() == "true" || v == "1")
                    .unwrap_or(false);

                let new_status = !current_status;
                std::env::set_var("GROK_ENABLE_INTERLEAVED_THINKING", new_status.to_string());

                state.session.add_system_message(format!(
                    "Interleaved thinking is now {}. This will take effect for the next conversation.\n• When enabled, the agent will share its reasoning process between tool calls\n• Use '/thinking' again to toggle",
                    if new_status { "enabled" } else { "disabled" }
                ));
                state.processing = false;
                return;
            }
            _ => {}
        }

        // Re-enable auto-scroll for new conversation
        state.auto_scroll_chat = true;

        // Process with session (this adds the user message immediately and
        // spawns a background task for the agent response)
        state.session.handle_user_input(input).await;
        // Keep `processing` true; it will be set to false when the
        // AgentResponse or AgentError event is received.
    }

    /// Insert a character at the cursor position
    fn insert_char(state: &mut AppState, ch: char) {
        if state.input_cursor <= state.input.len() {
            state.input.insert(state.input_cursor, ch);
            state.input_cursor += ch.len_utf8();
        }
    }

    /// Delete character before cursor
    fn delete_char(state: &mut AppState) {
        if state.input_cursor > 0 {
            state.input_cursor -= 1;
            state.input.remove(state.input_cursor);
        }
    }

    /// Move cursor left
    fn move_cursor_left(state: &mut AppState) {
        if state.input_cursor > 0 {
            // Move to previous character boundary
            let mut new_cursor = state.input_cursor - 1;
            while new_cursor > 0 && !state.input.is_char_boundary(new_cursor) {
                new_cursor -= 1;
            }
            state.input_cursor = new_cursor;
        }
    }

    /// Move cursor right
    fn move_cursor_right(state: &mut AppState) {
        if state.input_cursor < state.input.len() {
            // Move to next character boundary
            let mut new_cursor = state.input_cursor + 1;
            while new_cursor < state.input.len() && !state.input.is_char_boundary(new_cursor) {
                new_cursor += 1;
            }
            if new_cursor <= state.input.len() {
                state.input_cursor = new_cursor;
            }
        }
    }

    /// Move cursor up in multi-line input
    fn move_cursor_up(state: &mut AppState) {
        // For now, just move to beginning of current line or previous line
        // This is a simplified implementation - a full implementation would need
        // to calculate line positions properly
        if let Some(newline_pos) = state.input[..state.input_cursor].rfind('\n') {
            let current_line_start = newline_pos + 1;
            let current_col = state.input_cursor - current_line_start;

            // Find previous line
            if let Some(prev_newline_pos) = state.input[..newline_pos].rfind('\n') {
                let prev_line_start = prev_newline_pos + 1;
                let prev_line_len = newline_pos - prev_line_start;
                let target_col = current_col.min(prev_line_len);
                state.input_cursor = prev_line_start + target_col;
            } else {
                // First line
                let target_col = current_col.min(newline_pos);
                state.input_cursor = target_col;
            }
        } else {
            // First line, go to beginning
            state.input_cursor = 0;
        }
    }

    /// Move cursor down in multi-line input
    fn move_cursor_down(state: &mut AppState) {
        // For now, just move to end of current line or next line
        // This is a simplified implementation - a full implementation would need
        // to calculate line positions properly
        if let Some(newline_pos) = state.input[state.input_cursor..].find('\n') {
            let current_newline_pos = state.input_cursor + newline_pos;
            let next_line_start = current_newline_pos + 1;

            if next_line_start < state.input.len() {
                // Find end of next line
                if let Some(next_newline_pos) = state.input[next_line_start..].find('\n') {
                    let next_line_end = next_line_start + next_newline_pos;
                    let next_line_len = next_line_end - next_line_start;

                    // Calculate current column position
                    let current_line_start = state.input[..state.input_cursor]
                        .rfind('\n')
                        .map(|pos| pos + 1)
                        .unwrap_or(0);
                    let current_col = state.input_cursor - current_line_start;

                    let target_col = current_col.min(next_line_len);
                    state.input_cursor = next_line_start + target_col;
                } else {
                    // Last line
                    let current_line_start = state.input[..state.input_cursor]
                        .rfind('\n')
                        .map(|pos| pos + 1)
                        .unwrap_or(0);
                    let current_col = state.input_cursor - current_line_start;
                    let last_line_len = state.input.len() - next_line_start;
                    let target_col = current_col.min(last_line_len);
                    state.input_cursor = next_line_start + target_col;
                }
            } else {
                // No next line, go to end
                state.input_cursor = state.input.len();
            }
        } else {
            // Last line, go to end
            state.input_cursor = state.input.len();
        }
    }

    /// Get filtered commands based on current filter
    fn get_filtered_commands(state: &AppState) -> Vec<&crate::state::Command> {
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

    /// Handle character input for command palette filtering
    fn handle_command_palette_char(state: &mut AppState, c: char) {
        if c.is_alphanumeric() || c == '/' || c == ' ' || c == '-' || c == '_' {
            state.command_palette_filter.push(c);
            // Reset selection when filter changes
            state.command_palette_selected = 0;
        }
    }

    /// Execute the currently selected command
    async fn execute_selected_command(state: &mut AppState) {
        let filtered_commands = Self::get_filtered_commands(state);
        if let Some(cmd) = filtered_commands.get(state.command_palette_selected) {
            let command_text = cmd.name.clone();

            // Close the command palette
            state.command_palette_open = false;
            state.command_palette_filter.clear();
            state.command_palette_selected = 0;

            // Execute the command
            state.input = command_text;
            Self::submit_input(state).await;
        }
    }
}
