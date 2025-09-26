pub mod executor_tests;
pub mod fs_executor_tests;
pub mod shell_executor_tests;
pub mod code_executor_tests;
pub mod registry_tests;
pub mod types_tests;

// Test utilities
use crate::events::{AppEvent, EventBus};
use crate::tools::types::*;
use serde_json::Value;
use std::path::Path;
use tokio::fs;
use tokio::sync::mpsc;

/// Test helper to create a temporary directory
pub async fn create_temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().expect("Failed to create temp directory")
}

/// Test helper to create a temporary file with content
pub async fn create_temp_file(dir: &Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    fs::write(&path, content).await.expect("Failed to write temp file");
    path
}

/// Test helper to setup an event bus for testing
pub fn setup_event_bus() -> (crate::events::EventSender, mpsc::UnboundedReceiver<AppEvent>) {
    let bus = EventBus::new();
    let sender = bus.sender();
    let receiver = bus.into_receiver();
    (sender, receiver)
}

/// Test helper to collect events from a receiver
pub async fn collect_events(receiver: &mut mpsc::UnboundedReceiver<AppEvent>, count: usize) -> Vec<AppEvent> {
    let mut events = Vec::new();
    for _ in 0..count {
        if let Some(event) = receiver.recv().await {
            events.push(event);
        }
    }
    events
}

/// Test helper to find specific event type in a list
pub fn find_tool_result_event(events: &[AppEvent]) -> Option<Value> {
    for event in events {
        if let AppEvent::ToolResult { payload, .. } = event {
            return Some(payload.clone());
        }
    }
    None
}

/// Test helper to find tool end event
pub fn find_tool_end_event(events: &[AppEvent]) -> Option<(bool, u64)> {
    for event in events {
        if let AppEvent::ToolEnd { ok, duration_ms, .. } = event {
            return Some((*ok, *duration_ms));
        }
    }
    None
}

/// Test helper to count events of a specific type
pub fn count_progress_events(events: &[AppEvent]) -> usize {
    events.iter().filter(|e| matches!(e, AppEvent::ToolProgress { .. })).count()
}

pub fn count_stdout_events(events: &[AppEvent]) -> usize {
    events.iter().filter(|e| matches!(e, AppEvent::ToolStdout { .. })).count()
}

pub fn count_stderr_events(events: &[AppEvent]) -> usize {
    events.iter().filter(|e| matches!(e, AppEvent::ToolStderr { .. })).count()
}
