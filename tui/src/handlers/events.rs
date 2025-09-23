use grok_core::AppEvent;
use tracing::{debug, error};
use crate::state::AppState;

/// Handles application events from the session
pub struct EventHandler;

impl EventHandler {
    /// Handle application events
    pub async fn handle_event(state: &mut AppState, event: AppEvent) {
        debug!("Handling app event: {:?}", event);
        match event {
            AppEvent::UserInput(_) => {
                // User input is handled directly in submit_input
            }
            AppEvent::AgentResponse(response) => {
                // Append agent response and mark as done
                state.session.add_agent_message(response.content);
                state.processing = false;
                // Re-enable auto-scroll for new content
                state.auto_scroll_chat = true;
                debug!("Received agent response");
            }
            AppEvent::AgentError(error) => {
                state.session.add_error_message(format!("{}", error));
                state.processing = false;
                error!("Agent error: {}", error);
            }
            AppEvent::AgentThinking(thinking) => {
                // Add thinking message to the session and enable auto-scroll
                state.session.handle_agent_thinking(thinking);
                state.auto_scroll_chat = true;
                debug!("Received agent thinking step");
            }
            AppEvent::Quit => {
                state.should_quit = true;
            }
            AppEvent::Clear => {
                state.session.clear();
                // Reset UI state to fresh start
                state.chat_scroll = 0;
                state.tools_scroll = 0;
                state.auto_scroll_chat = true;
                state.auto_scroll_tools = true;
                state.current_token_usage = None;
            }
            AppEvent::ShowAgentInfo => {
                let info = state.session.agent_info();
                state.session.add_system_message(format!(
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
                state.processing = false;
            }

            // Tool lifecycle events
            AppEvent::ToolBegin { id, tool, summary, args } => {
                debug!("Tool {} started: {}", id, summary);
                
                // Add a system message to chat panel indicating tool usage
                let tool_display_name = match tool {
                    grok_core::ToolName::FsRead => "file reader",
                    grok_core::ToolName::FsSearch => "file search",
                    grok_core::ToolName::FsWrite => "file writer",
                    grok_core::ToolName::FsApplyPatch => "patch applicator",
                    grok_core::ToolName::FsFind => "file finder",
                    grok_core::ToolName::FsReadAllCode => "code reader",
                    grok_core::ToolName::ShellExec => "shell command",
                    grok_core::ToolName::CodeSymbols => "code analyzer",
                };
                state.session.add_system_message(format!("Agent ran {} tool", tool_display_name));
                
                state.session.handle_tool_begin(id, tool, summary, args);
                // Re-enable auto-scroll for new tools and chat
                state.auto_scroll_tools = true;
                state.auto_scroll_chat = true;
            }
            AppEvent::ToolProgress { id, message } => {
                debug!("Tool {} progress: {}", id, message);
                state.session.handle_tool_progress(id, message);
            }
            AppEvent::ToolStdout { id, chunk } => {
                debug!("Tool {} stdout: {}", id, chunk);
                state.session.handle_tool_stdout(id, chunk);
            }
            AppEvent::ToolStderr { id, chunk } => {
                debug!("Tool {} stderr: {}", id, chunk);
                state.session.handle_tool_stderr(id, chunk);
            }
            AppEvent::ToolResult { id, payload } => {
                debug!("Tool {} result: {:?}", id, payload);
                state.session.handle_tool_result(id, payload);
            }
            AppEvent::ToolEnd { id, ok, duration_ms } => {
                debug!("Tool {} ended: ok={}, duration={}ms", id, ok, duration_ms);
                state.session.handle_tool_end(id, ok, duration_ms);
            }

            // Safety/approval events
            AppEvent::ApprovalRequest { id: _, tool, summary } => {
                debug!("Approval requested for tool {:?}: {}", tool, summary);
                // For mock implementation, auto-approve
                // In real implementation, show approval UI
                state.session.add_system_message(format!("Tool {:?} needs approval: {}", tool, summary));
            }
            AppEvent::ApprovalDecision { id, approved } => {
                debug!("Approval decision for {}: {}", id, approved);
            }

            // Error and background events
            AppEvent::Error { id: _, message } => {
                error!("Error: {}", message);
                state.session.add_error_message(format!("Error: {}", message));
            }
            AppEvent::TokenCount(usage) => {
                debug!("Token usage: {}/{} tokens", usage.input_tokens, usage.output_tokens);
                // Update current token usage for the /context command
                state.current_token_usage = Some(usage);
            }
            AppEvent::Background(message) => {
                debug!("Background: {}", message);
            }
        }
    }
}
