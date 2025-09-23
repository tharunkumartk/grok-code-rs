use super::*;
use crate::tools::ToolExecutor;
use crate::events::ToolName;
use serde_json::json;

#[tokio::test]
async fn test_tool_executor_creation() {
    let (sender, _receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    // Test with custom max output size
    let _custom_executor = executor.with_max_output_size(512);
    
    // Just verify the executor can be created without panicking
    assert!(true);
}

#[tokio::test]
async fn test_tool_executor_fs_read() {
    let temp_dir = create_temp_dir().await;
    let test_content = "Test file content";
    let file_path = create_temp_file(temp_dir.path(), "test.txt", test_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "encoding": "utf-8"
    });
    
    let result = executor.execute_tool_with_result(
        "test_id".to_string(),
        ToolName::FsRead,
        args
    ).await;
    
    assert!(result.is_ok());
    let result_value = result.unwrap();
    let fs_result: FsReadResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(fs_result.contents, test_content);
    
    // Check events: ToolBegin, ToolProgress, ToolResult, ToolEnd
    let events = collect_events(&mut receiver, 4).await;
    
    // Find ToolBegin event
    let begin_event = events.iter().find(|e| matches!(e, AppEvent::ToolBegin { .. }));
    assert!(begin_event.is_some());
    
    // Find ToolEnd event and verify success
    let (ok, duration) = find_tool_end_event(&events).unwrap();
    assert!(ok);
    // Duration should be a valid number (u64 is always >= 0)
    assert!(duration < u64::MAX);
}

#[tokio::test]
async fn test_tool_executor_fs_write() {
    let temp_dir = create_temp_dir().await;
    let file_path = temp_dir.path().join("new_file.txt");
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    let test_content = "New file content";
    let args = json!({
        "path": file_path.to_string_lossy(),
        "contents": test_content,
        "create_if_missing": true,
        "overwrite": false
    });
    
    let result = executor.execute_tool_with_result(
        "test_id".to_string(),
        ToolName::FsWrite,
        args
    ).await;
    
    assert!(result.is_ok());
    
    // Verify file was written
    let written_content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(written_content, test_content);
    
    let events = collect_events(&mut receiver, 4).await;
    let (ok, _) = find_tool_end_event(&events).unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_tool_executor_fs_search() {
    // Create test file in current directory since fs_search searches from "."
    let test_file = "temp_executor_test.rs";
    tokio::fs::write(test_file, "fn main() {}").await.expect("Failed to create test file");
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    let args = json!({
        "query": "fn",
        "globs": ["*.rs"],
        "regex": false,
        "case_insensitive": false,
        "multiline": false,
        "max_results": 10
    });
    
    let result = executor.execute_tool_with_result(
        "test_id".to_string(),
        ToolName::FsSearch,
        args
    ).await;
    
    assert!(result.is_ok());
    let result_value = result.unwrap();
    let _search_result: FsSearchResult = serde_json::from_value(result_value).unwrap();
    // May or may not find results depending on existing .rs files in the directory
    // Just check that the operation completed successfully
    
    // Cleanup
    let _ = tokio::fs::remove_file(test_file).await;
    
    let events = collect_events(&mut receiver, 4).await;
    let (ok, _) = find_tool_end_event(&events).unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_tool_executor_fs_find() {
    // Create test file in current directory
    let test_file = "temp_executor_find_test.rs";
    tokio::fs::write(test_file, "fn main() {}").await.expect("Failed to create test file");
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    let args = json!({
        "pattern": "temp_executor_find_test.rs",
        "base_path": ".",
        "fuzzy": false,
        "case_sensitive": false,
        "file_type": "file",
        "max_results": 10
    });
    
    let result = executor.execute_tool_with_result(
        "test_id".to_string(),
        ToolName::FsFind,
        args
    ).await;
    
    assert!(result.is_ok());
    let result_value = result.unwrap();
    let find_result: FsFindResult = serde_json::from_value(result_value).unwrap();
    assert!(find_result.matches.len() >= 1);
    let found_file = find_result.matches.iter().any(|m| m.path.contains("temp_executor_find_test.rs"));
    assert!(found_file);
    
    // Cleanup
    let _ = tokio::fs::remove_file(test_file).await;
    
    let events = collect_events(&mut receiver, 4).await;
    let (ok, _) = find_tool_end_event(&events).unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_tool_executor_fs_apply_patch() {
    let temp_dir = create_temp_dir().await;
    let _file_path = create_temp_file(temp_dir.path(), "test.rs", "fn main() {}").await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    let patch = format!(
        "--- {}/test.rs\n+++ {}/test.rs\n@@ -1,1 +1,1 @@\n-fn main() {{}}\n+fn main() {{ println!(\"Hello\"); }}",
        temp_dir.path().display(),
        temp_dir.path().display()
    );
    
    let args = json!({
        "unified_diff": patch,
        "dry_run": true
    });
    
    let result = executor.execute_tool_with_result(
        "test_id".to_string(),
        ToolName::FsApplyPatch,
        args
    ).await;
    
    assert!(result.is_ok());
    let result_value = result.unwrap();
    let patch_result: FsApplyPatchResult = serde_json::from_value(result_value).unwrap();
    assert!(patch_result.success);
    
    let events = collect_events(&mut receiver, 5).await; // More events for patch (2 progress)
    let (ok, _) = find_tool_end_event(&events).unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_tool_executor_shell_exec() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    let args = json!({
        "command": ["echo", "test message"],
        "timeout_ms": 5000
    });
    
    let result = executor.execute_tool_with_result(
        "test_id".to_string(),
        ToolName::ShellExec,
        args
    ).await;
    
    assert!(result.is_ok());
    let result_value = result.unwrap();
    let shell_result: ShellExecResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(shell_result.exit_code, 0);
    assert!(shell_result.stdout.contains("test message"));
    
    let events = collect_events(&mut receiver, 5).await; // Begin, Progress, Stdout, Result, End
    let (ok, _) = find_tool_end_event(&events).unwrap();
    assert!(ok);
    assert!(count_stdout_events(&events) >= 1);
}

#[tokio::test]
async fn test_tool_executor_code_symbols() {
    let temp_dir = create_temp_dir().await;
    let rust_content = "fn main() {}\nstruct Test {}";
    let file_path = create_temp_file(temp_dir.path(), "test.rs", rust_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "language": "rust"
    });
    
    let result = executor.execute_tool_with_result(
        "test_id".to_string(),
        ToolName::CodeSymbols,
        args
    ).await;
    
    assert!(result.is_ok());
    let result_value = result.unwrap();
    let symbols_result: CodeSymbolsResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(symbols_result.language, "rust");
    assert!(!symbols_result.symbols.is_empty());
    
    let events = collect_events(&mut receiver, 4).await;
    let (ok, _) = find_tool_end_event(&events).unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_tool_executor_legacy_methods() {
    let temp_dir = create_temp_dir().await;
    let test_content = "Legacy test content";
    let file_path = create_temp_file(temp_dir.path(), "test.txt", test_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "encoding": "utf-8"
    });
    
    // Test legacy execute_tool method (no result return)
    let result = executor.execute_tool(
        "test_id".to_string(),
        ToolName::FsRead,
        args
    ).await;
    
    assert!(result.is_ok());
    
    let events = collect_events(&mut receiver, 4).await;
    let (ok, _) = find_tool_end_event(&events).unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_tool_executor_error_handling() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    // Test with nonexistent file
    let args = json!({
        "path": "/nonexistent/file.txt",
        "encoding": "utf-8"
    });
    
    let result = executor.execute_tool_with_result(
        "test_id".to_string(),
        ToolName::FsRead,
        args
    ).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("File not found"));
    
    let events = collect_events(&mut receiver, 3).await; // Begin, Progress, End
    let (ok, _) = find_tool_end_event(&events).unwrap();
    assert!(!ok); // Should indicate failure
}

#[tokio::test]
async fn test_tool_executor_shell_failure() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    let args = json!({
        "command": ["false"], // Command that returns exit code 1
        "timeout_ms": 5000
    });
    
    let result = executor.execute_tool_with_result(
        "test_id".to_string(),
        ToolName::ShellExec,
        args
    ).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Command failed with exit code: 1"));
    
    let events = collect_events(&mut receiver, 4).await; // Begin, Progress, Result, End
    let (ok, _) = find_tool_end_event(&events).unwrap();
    assert!(!ok);
}

#[tokio::test]
async fn test_tool_executor_custom_max_output_size() {
    let temp_dir = create_temp_dir().await;
    let large_content = "x".repeat(2000); // 2KB content
    let file_path = create_temp_file(temp_dir.path(), "large.txt", &large_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender).with_max_output_size(500); // Small limit
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "encoding": "utf-8"
    });
    
    let result = executor.execute_tool_with_result(
        "test_id".to_string(),
        ToolName::FsRead,
        args
    ).await;
    
    assert!(result.is_ok());
    let result_value = result.unwrap();
    
    // Result should be truncated
    assert!(result_value.get("truncated").is_some());
    assert_eq!(result_value["truncated"], true);
    
    let events = collect_events(&mut receiver, 4).await;
    let (ok, _) = find_tool_end_event(&events).unwrap();
    assert!(ok);
}

#[tokio::test]
async fn test_tool_executor_concurrent_execution() {
    let temp_dir = create_temp_dir().await;
    let file1_path = create_temp_file(temp_dir.path(), "file1.txt", "content1").await;
    let file2_path = create_temp_file(temp_dir.path(), "file2.txt", "content2").await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = ToolExecutor::new(sender);
    
    let args1 = json!({
        "path": file1_path.to_string_lossy(),
        "encoding": "utf-8"
    });
    
    let args2 = json!({
        "path": file2_path.to_string_lossy(),
        "encoding": "utf-8"
    });
    
    // Execute tools concurrently
    let (result1, result2) = tokio::join!(
        executor.execute_tool_with_result("test_id1".to_string(), ToolName::FsRead, args1),
        executor.execute_tool_with_result("test_id2".to_string(), ToolName::FsRead, args2)
    );
    
    assert!(result1.is_ok());
    assert!(result2.is_ok());
    
    let fs_result1: FsReadResult = serde_json::from_value(result1.unwrap()).unwrap();
    let fs_result2: FsReadResult = serde_json::from_value(result2.unwrap()).unwrap();
    
    assert_eq!(fs_result1.contents, "content1");
    assert_eq!(fs_result2.contents, "content2");
    
    // Should receive events for both executions
    let events = collect_events(&mut receiver, 8).await; // 4 events per execution
    
    // Count events by ID
    let id1_events: Vec<_> = events.iter().filter(|e| {
        match e {
            AppEvent::ToolBegin { id, .. } => id == "test_id1",
            AppEvent::ToolProgress { id, .. } => id == "test_id1",
            AppEvent::ToolResult { id, .. } => id == "test_id1",
            AppEvent::ToolEnd { id, .. } => id == "test_id1",
            _ => false,
        }
    }).collect();
    
    let id2_events: Vec<_> = events.iter().filter(|e| {
        match e {
            AppEvent::ToolBegin { id, .. } => id == "test_id2",
            AppEvent::ToolProgress { id, .. } => id == "test_id2",
            AppEvent::ToolResult { id, .. } => id == "test_id2",
            AppEvent::ToolEnd { id, .. } => id == "test_id2",
            _ => false,
        }
    }).collect();
    
    assert_eq!(id1_events.len(), 4);
    assert_eq!(id2_events.len(), 4);
}
