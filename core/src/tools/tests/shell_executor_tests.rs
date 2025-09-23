use super::*;
use crate::tools::executors::ShellExecutor;
use serde_json::json;

#[tokio::test]
async fn test_shell_exec_success() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "command": ["echo", "Hello, World!"],
        "timeout_ms": 5000
    });
    
    let result = executor.execute_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let shell_result: ShellExecResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(shell_result.exit_code, 0);
    assert!(shell_result.stdout.contains("Hello, World!"));
    assert!(shell_result.stderr.is_empty());
    assert!(shell_result.duration_ms > 0);
    
    // Should have progress event, stdout events, and result event
    let events = collect_events(&mut receiver, 3).await;
    assert_eq!(count_progress_events(&events), 1);
    assert!(count_stdout_events(&events) >= 1);
    assert!(find_tool_result_event(&events).is_some());
}

#[tokio::test]
async fn test_shell_exec_with_args() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "command": ["echo", "-n", "no", "newline"],
        "timeout_ms": 5000
    });
    
    let result = executor.execute_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let shell_result: ShellExecResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(shell_result.exit_code, 0);
    assert!(shell_result.stdout.contains("no newline"));
    
    let events = collect_events(&mut receiver, 3).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_shell_exec_stderr_output() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    // Use a command that outputs to stderr
    let args = json!({
        "command": ["sh", "-c", "echo 'error message' >&2"],
        "timeout_ms": 5000
    });
    
    let result = executor.execute_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let shell_result: ShellExecResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(shell_result.exit_code, 0);
    assert!(shell_result.stderr.contains("error message"));
    
    let events = collect_events(&mut receiver, 3).await;
    assert_eq!(count_progress_events(&events), 1);
    assert!(count_stderr_events(&events) >= 1);
}

#[tokio::test]
async fn test_shell_exec_failure() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    // Use a command that will fail
    let args = json!({
        "command": ["false"], // Command that always returns exit code 1
        "timeout_ms": 5000
    });
    
    let result = executor.execute_with_result("test_id".to_string(), args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Command failed with exit code: 1"));
    
    let events = collect_events(&mut receiver, 2).await; // progress + result
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_shell_exec_nonexistent_command() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "command": ["nonexistent_command_12345"],
        "timeout_ms": 5000
    });
    
    let result = executor.execute_with_result("test_id".to_string(), args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Failed to spawn command"));
    
    let events = collect_events(&mut receiver, 1).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_shell_exec_empty_command() {
    let (sender, _receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "command": [],
        "timeout_ms": 5000
    });
    
    let result = executor.execute_with_result("test_id".to_string(), args).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Empty command");
}

#[tokio::test]
async fn test_shell_exec_with_cwd() {
    let temp_dir = create_temp_dir().await;
    let test_file_path = create_temp_file(temp_dir.path(), "test_file.txt", "test content").await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "command": ["ls", "test_file.txt"],
        "cwd": temp_dir.path().to_string_lossy(),
        "timeout_ms": 5000
    });
    
    let result = executor.execute_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let shell_result: ShellExecResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(shell_result.exit_code, 0);
    assert!(shell_result.stdout.contains("test_file.txt"));
    
    let events = collect_events(&mut receiver, 3).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_shell_exec_with_env_vars() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "command": ["sh", "-c", "echo $TEST_VAR"],
        "env": [["TEST_VAR", "test_value"]],
        "timeout_ms": 5000
    });
    
    let result = executor.execute_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let shell_result: ShellExecResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(shell_result.exit_code, 0);
    assert!(shell_result.stdout.contains("test_value"));
    
    let events = collect_events(&mut receiver, 3).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_shell_exec_timeout() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    // Command that takes longer than timeout
    let args = json!({
        "command": ["sleep", "2"],
        "timeout_ms": 100 // Very short timeout
    });
    
    let result = executor.execute_with_result("test_id".to_string(), args).await;
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Command timed out");
    
    let events = collect_events(&mut receiver, 1).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_shell_exec_legacy_method() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "command": ["echo", "legacy test"],
        "timeout_ms": 5000
    });
    
    // Test the legacy execute method (no result return)
    let result = executor.execute("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    // Legacy method should still send events
    let events = collect_events(&mut receiver, 3).await;
    assert_eq!(count_progress_events(&events), 1);
    assert!(count_stdout_events(&events) >= 1);
    assert!(find_tool_result_event(&events).is_some());
}

#[tokio::test]
async fn test_shell_exec_invalid_args() {
    let (sender, _receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    let invalid_args = json!({
        "invalid_field": "value"
    });
    
    let result = executor.execute_with_result("test_id".to_string(), invalid_args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid ShellExec arguments"));
}

#[tokio::test]
async fn test_shell_exec_output_truncation() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 100); // Very small max output
    
    // Generate a lot of output
    let args = json!({
        "command": ["sh", "-c", "for i in $(seq 1 100); do echo 'This is line $i with some content'; done"],
        "timeout_ms": 5000
    });
    
    let result = executor.execute_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    // Result should be truncated due to small max_output_size
    assert!(result_value.get("truncated").is_some());
    
    let events = collect_events(&mut receiver, 10).await; // Many stdout events + others
    assert_eq!(count_progress_events(&events), 1);
    assert!(count_stdout_events(&events) > 1);
}

#[tokio::test]
async fn test_shell_exec_complex_command() {
    let temp_dir = create_temp_dir().await;
    let _file1 = create_temp_file(temp_dir.path(), "file1.txt", "content1").await;
    let _file2 = create_temp_file(temp_dir.path(), "file2.txt", "content2").await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = ShellExecutor::new(sender, 1024 * 1024);
    
    // Complex shell command with pipes
    let args = json!({
        "command": ["sh", "-c", "ls *.txt | wc -l"],
        "cwd": temp_dir.path().to_string_lossy(),
        "timeout_ms": 5000
    });
    
    let result = executor.execute_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let shell_result: ShellExecResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(shell_result.exit_code, 0);
    assert!(shell_result.stdout.trim() == "2"); // Should count 2 .txt files
    
    let events = collect_events(&mut receiver, 3).await;
    assert_eq!(count_progress_events(&events), 1);
}
