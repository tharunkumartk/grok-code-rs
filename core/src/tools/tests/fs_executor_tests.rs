use super::*;
use crate::tools::executors::FsExecutor;
use serde_json::json;

#[tokio::test]
async fn test_fs_read_success() {
    let temp_dir = create_temp_dir().await;
    let test_content = "Hello, World!\nThis is a test file.";
    let file_path = create_temp_file(temp_dir.path(), "test.txt", test_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "encoding": "utf-8"
    });
    
    let result = executor.execute_read_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let fs_result: FsReadResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(fs_result.contents, test_content);
    assert_eq!(fs_result.encoding, "utf-8");
    assert!(!fs_result.truncated);
    
    // Check events
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
    assert!(find_tool_result_event(&events).is_some());
}

#[tokio::test]
async fn test_fs_read_file_not_found() {
    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": "/nonexistent/file.txt",
        "encoding": "utf-8"
    });
    
    let result = executor.execute_read_with_result("test_id".to_string(), args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("File not found"));
    
    // Should only have progress event, no result event on error
    let events = collect_events(&mut receiver, 1).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_fs_read_with_range() {
    let temp_dir = create_temp_dir().await;
    let test_content = "0123456789abcdefghijklmnopqrstuvwxyz";
    let file_path = create_temp_file(temp_dir.path(), "test.txt", test_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "range": { "start": 5, "end": 15 },
        "encoding": "utf-8"
    });
    
    let result = executor.execute_read_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let fs_result: FsReadResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(fs_result.contents, "56789abcde");
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_fs_write_success() {
    let temp_dir = create_temp_dir().await;
    let file_path = temp_dir.path().join("new_file.txt");
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);
    
    let test_content = "This is new content";
    let args = json!({
        "path": file_path.to_string_lossy(),
        "contents": test_content,
        "create_if_missing": true,
        "overwrite": false
    });
    
    let result = executor.execute_write_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let fs_result: FsWriteResult = serde_json::from_value(result_value).unwrap();
    assert_eq!(fs_result.bytes_written, test_content.len() as u64);
    
    // Verify file was actually written
    let written_content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(written_content, test_content);
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
    assert!(find_tool_result_event(&events).is_some());
}

#[tokio::test]
async fn test_fs_write_file_exists_no_overwrite() {
    let temp_dir = create_temp_dir().await;
    let file_path = create_temp_file(temp_dir.path(), "existing.txt", "original content").await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "contents": "new content",
        "create_if_missing": true,
        "overwrite": false
    });
    
    let result = executor.execute_write_with_result("test_id".to_string(), args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("File already exists and overwrite is false"));
    
    // Original file should be unchanged
    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(content, "original content");
    
    let events = collect_events(&mut receiver, 1).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_fs_write_with_overwrite() {
    let temp_dir = create_temp_dir().await;
    let file_path = create_temp_file(temp_dir.path(), "existing.txt", "original content").await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);
    
    let new_content = "completely new content";
    let args = json!({
        "path": file_path.to_string_lossy(),
        "contents": new_content,
        "create_if_missing": true,
        "overwrite": true
    });
    
    let result = executor.execute_write_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    // Verify file was overwritten
    let content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(content, new_content);
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_fs_write_create_directories() {
    let temp_dir = create_temp_dir().await;
    let nested_path = temp_dir.path().join("some").join("nested").join("dirs").join("file.txt");
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "path": nested_path.to_string_lossy(),
        "contents": "nested file content",
        "create_if_missing": true,
        "overwrite": false
    });
    
    let result = executor.execute_write_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    // Verify directories were created and file exists
    assert!(nested_path.exists());
    let content = tokio::fs::read_to_string(&nested_path).await.unwrap();
    assert_eq!(content, "nested file content");
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_fs_search_success() {
    // Create test files in current directory since fs_search searches from "."
    let test_file = "temp_test_file1.rs";
    let test_content = "fn main() {\n    println!(\"Hello, world!\");\n}";
    tokio::fs::write(test_file, test_content).await.expect("Failed to create test file");
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "query": "fn",
        "globs": ["*.rs"],
        "regex": false,
        "case_insensitive": false,
        "multiline": false,
        "max_results": 10
    });
    
    let result = executor.execute_search_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let search_result: FsSearchResult = serde_json::from_value(result_value).unwrap();
    
    // Should find "fn" in the test file
    assert!(!search_result.matches.is_empty());
    let found_file = search_result.matches.iter()
        .any(|m| m.path.contains(test_file));
    assert!(found_file);
    
    // Cleanup
    let _ = tokio::fs::remove_file(test_file).await;
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_fs_search_regex() {
    // Create test file in current directory
    let test_file = "temp_test_emails.txt";
    let test_content = "Email: test@example.com\nAnother: user@domain.org\nNot an email: example.com";
    tokio::fs::write(test_file, test_content).await.expect("Failed to create test file");
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "query": r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b",
        "regex": true,
        "case_insensitive": false,
        "multiline": false,
        "max_results": 10
    });
    
    let result = executor.execute_search_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let search_result: FsSearchResult = serde_json::from_value(result_value).unwrap();
    
    // Should find email addresses
    assert!(!search_result.matches.is_empty());
    if !search_result.matches.is_empty() {
        let file_match = &search_result.matches[0];
        // Should have at least one email line
        assert!(!file_match.lines.is_empty());
    }
    
    // Cleanup
    let _ = tokio::fs::remove_file(test_file).await;
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}

#[tokio::test]
async fn test_fs_find_success() {
    // Create test files in current directory
    let test_files = ["temp_main.rs", "temp_lib.rs", "temp_test.txt"];
    let contents = ["fn main() {}", "pub mod lib {}", "text file"];
    
    for (file, content) in test_files.iter().zip(contents.iter()) {
        tokio::fs::write(file, content).await.expect("Failed to create test file");
    }
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);
    
    let args = json!({
        "pattern": "temp_*.rs",
        "base_path": ".",
        "fuzzy": false,
        "case_sensitive": false,
        "file_type": "file",
        "max_results": 10
    });
    
    let result = executor.execute_find_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    let find_result: FsFindResult = serde_json::from_value(result_value).unwrap();
    
    // Should find 2 .rs files with temp_ prefix
    assert!(find_result.matches.len() >= 2);
    let rs_files: Vec<_> = find_result.matches.iter()
        .filter(|m| m.path.contains("temp_") && m.path.ends_with(".rs"))
        .collect();
    assert!(rs_files.len() >= 2);
    
    // Cleanup
    for file in &test_files {
        let _ = tokio::fs::remove_file(file).await;
    }
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}


#[tokio::test]
async fn test_fs_apply_patch_dry_run() {
    let temp_dir = create_temp_dir().await;
    let file_path = create_temp_file(
        temp_dir.path(),
        "test.rs",
        r#"fn main() {
    println!("Hello");
}"#,
    ).await;

    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);

    let spec = FsApplyPatchArgs {
        dry_run: true,
        ops: vec![SimpleEditOp::ReplaceOnce {
            path: file_path.to_string_lossy().to_string(),
            find: "println!(\"Hello\");".to_string(),
            replace: "println!(\"Hello, World!\");".to_string(),
        }],
    };
    let args = serde_json::to_value(spec).unwrap();

    let result = executor.execute_apply_patch_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());

    let result_value = result.unwrap();
    let patch_result: FsApplyPatchResult = serde_json::from_value(result_value).unwrap();
    assert!(patch_result.success);
    assert!(patch_result.summary.contains("Dry run"));

    let events = collect_events(&mut receiver, 3).await; // 2 progress + 1 result
    assert_eq!(count_progress_events(&events), 2);
}


#[tokio::test]
async fn test_fs_apply_patch_invalid_format() {
    let temp_dir = create_temp_dir().await;
    let file_path = create_temp_file(temp_dir.path(), "test.rs", "fn main() {}
").await;

    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);

    let spec = FsApplyPatchArgs {
        dry_run: false,
        ops: vec![SimpleEditOp::ReplaceOnce {
            path: file_path.to_string_lossy().to_string(),
            find: "this pattern does not exist".to_string(),
            replace: "fn main() { unreachable!(); }".to_string(),
        }],
    };
    let args = serde_json::to_value(spec).unwrap();

    let result = executor.execute_apply_patch_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok()); // Function succeeds but operation fails

    let result_value = result.unwrap();
    let patch_result: FsApplyPatchResult = serde_json::from_value(result_value).unwrap();
    assert!(!patch_result.success);
    assert!(patch_result.summary.contains("Failed to apply edits"));
    assert!(patch_result.rejected_hunks.is_some());

    let events = collect_events(&mut receiver, 3).await;
    assert_eq!(count_progress_events(&events), 2);
}


#[tokio::test]
async fn test_fs_apply_patch_real_modification() {
    let temp_dir = create_temp_dir().await;
    let original_content = r#"fn main() {
    let name = "World";
    println!("Hello, {}!", name);
    // TODO: Add more functionality
}"#;

    let file_path = create_temp_file(temp_dir.path(), "hello.rs", original_content).await;

    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);

    let original = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(original, original_content);
    assert!(original.contains("World"));
    assert!(!original.contains("Rust"));

    let path_str = file_path.to_string_lossy().to_string();
    let spec = FsApplyPatchArgs {
        dry_run: false,
        ops: vec![
            SimpleEditOp::ReplaceOnce {
                path: path_str.clone(),
                find: "let name = \"World\";".to_string(),
                replace: "let name = \"Rust\";".to_string(),
            },
            SimpleEditOp::InsertAfter {
                path: path_str.clone(),
                anchor: "println!(\"Hello, {}!\", name);".to_string(),
                insert: "\n    greet_user();".to_string(),
            },
            SimpleEditOp::InsertAfter {
                path: path_str,
                anchor: "    // TODO: Add more functionality\n}".to_string(),
                insert: "\n\nfn greet_user() {\n    println!(\"Welcome to Rust programming!\");\n}".to_string(),
            },
        ],
    };
    let args = serde_json::to_value(spec).unwrap();

    let result = executor.execute_apply_patch_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());

    let result_value = result.unwrap();
    let patch_result: FsApplyPatchResult = serde_json::from_value(result_value).unwrap();
    assert!(patch_result.success, "Patch should succeed: {}", patch_result.summary);
    assert!(patch_result.rejected_hunks.is_none() || patch_result.rejected_hunks.as_ref().unwrap().is_empty());

    let modified_content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_ne!(modified_content, original_content, "File content should have changed");
    assert!(!modified_content.contains("World"), "Old content should be replaced");
    assert!(modified_content.contains("Rust"), "New content should be present");
    assert!(modified_content.contains("greet_user"), "New function should be added");
    assert!(modified_content.contains("Welcome to Rust programming!"), "New function body should be present");

    let events = collect_events(&mut receiver, 3).await;
    assert_eq!(count_progress_events(&events), 2);
    assert!(find_tool_result_event(&events).is_some());
}


#[tokio::test]
async fn test_fs_apply_patch_create_new_file() {
    let temp_dir = create_temp_dir().await;
    let new_file_path = temp_dir.path().join("new_file.py");

    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);

    // Verify file doesn't exist initially
    assert!(!new_file_path.exists());

    let spec = FsApplyPatchArgs {
        dry_run: false,
        ops: vec![SimpleEditOp::SetFile {
            path: new_file_path.to_string_lossy().to_string(),
            contents: r#"#!/usr/bin/env python3

def hello_world():
    print("Hello from a new Python file!")

hello_world()
"#.to_string(),
        }],
    };
    let args = serde_json::to_value(spec).unwrap();

    let result = executor.execute_apply_patch_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());

    let result_value = result.unwrap();
    let patch_result: FsApplyPatchResult = serde_json::from_value(result_value).unwrap();
    assert!(patch_result.success, "Patch should succeed: {}", patch_result.summary);

    // Verify the new file was created with correct content
    assert!(new_file_path.exists(), "New file should have been created");
    let content = tokio::fs::read_to_string(&new_file_path).await.unwrap();
    assert!(content.contains("#!/usr/bin/env python3"));
    assert!(content.contains("def hello_world():"));
    assert!(content.contains("Hello from a new Python file!"));
    assert!(content.contains("hello_world()"));

    let events = collect_events(&mut receiver, 3).await;
    assert_eq!(count_progress_events(&events), 2);
}


#[tokio::test]
async fn test_fs_apply_patch_delete_file() {
    let temp_dir = create_temp_dir().await;
    let file_content = "This file will be deleted by the patch.";
    let file_path = create_temp_file(temp_dir.path(), "to_delete.txt", file_content).await;

    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);

    // Verify file exists initially
    assert!(file_path.exists());
    let original = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(original, file_content);

    let spec = FsApplyPatchArgs {
        dry_run: false,
        ops: vec![SimpleEditOp::DeleteFile {
            path: file_path.to_string_lossy().to_string(),
        }],
    };
    let args = serde_json::to_value(spec).unwrap();

    let result = executor.execute_apply_patch_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());

    let result_value = result.unwrap();
    let patch_result: FsApplyPatchResult = serde_json::from_value(result_value).unwrap();
    assert!(patch_result.success, "Patch should succeed: {}", patch_result.summary);

    // Verify the file was actually deleted
    assert!(!file_path.exists(), "File should have been deleted");

    let events = collect_events(&mut receiver, 3).await;
    assert_eq!(count_progress_events(&events), 2);
}

#[tokio::test]
async fn test_invalid_json_args() {
    let (sender, _receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1024 * 1024);
    
    // Invalid JSON for read
    let invalid_args = json!({
        "invalid_field": "value"
    });
    
    let result = executor.execute_read_with_result("test_id".to_string(), invalid_args).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid FsRead arguments"));
}

#[tokio::test]
async fn test_output_truncation() {
    let temp_dir = create_temp_dir().await;
    // Create a large file content that exceeds the max output size
    let large_content = "x".repeat(2000); // 2KB content
    let file_path = create_temp_file(temp_dir.path(), "large.txt", &large_content).await;
    
    let (sender, mut receiver) = setup_event_bus();
    let executor = FsExecutor::new(sender, 1000); // 1KB max output
    
    let args = json!({
        "path": file_path.to_string_lossy(),
        "encoding": "utf-8"
    });
    
    let result = executor.execute_read_with_result("test_id".to_string(), args).await;
    assert!(result.is_ok());
    
    let result_value = result.unwrap();
    // Result should be truncated
    assert!(result_value.get("truncated").is_some());
    assert_eq!(result_value["truncated"], true);
    
    let events = collect_events(&mut receiver, 2).await;
    assert_eq!(count_progress_events(&events), 1);
}
