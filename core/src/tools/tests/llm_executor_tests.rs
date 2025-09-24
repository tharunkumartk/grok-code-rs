use crate::events::{EventBus, ToolName};
use crate::tools::executors::LlmExecutor;
use crate::tools::types::LargeContextFetchArgs;
use serde_json::json;
use std::env;
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_large_context_fetch_args_validation() {
    let bus = EventBus::new();
    let _executor = LlmExecutor::new(bus.sender(), 1024 * 1024);

    let args = json!({
        "user_query": "How does authentication work in this codebase?",
        "base_path": ".",
        "max_files": 50
    });

    // Test argument validation by trying to parse
    let parsed_args: Result<LargeContextFetchArgs, _> = serde_json::from_value(args);
    assert!(parsed_args.is_ok());

    let parsed = parsed_args.unwrap();
    assert_eq!(parsed.user_query, "How does authentication work in this codebase?");
    assert_eq!(parsed.base_path, Some(".".to_string()));
    assert_eq!(parsed.max_files, Some(50));
}

#[tokio::test]
async fn test_large_context_fetch_missing_api_key() {
    let bus = EventBus::new();
    let executor = LlmExecutor::new(bus.sender(), 1024 * 1024);

    // Clear API key env vars for this test
    env::remove_var("OPENROUTER_API_KEY");
    env::remove_var("VERCEL_AI_GATEWAY_API_KEY");

    let args = json!({
        "user_query": "test query",
        "base_path": ".",
        "max_files": 5
    });

    let result = executor.execute_large_context_fetch_with_result("test_id".to_string(), args).await;
    
    // Should fail due to missing API key
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("No API key found"));
}

#[test]
fn test_number_extraction() {
    let bus = EventBus::new();
    let executor = LlmExecutor::new(bus.sender(), 1024 * 1024);
    
    // Test the number extraction logic
    let text = "Files 1, 5, and 12 are relevant. Also consider file 23.";
    let numbers = executor.extract_numbers_from_text(text);
    
    assert!(numbers.contains(&1));
    assert!(numbers.contains(&5));
    assert!(numbers.contains(&12));
    assert!(numbers.contains(&23));
}

#[test]
fn test_tool_name_in_registry() {
    use crate::tools::ToolRegistry;
    
    let registry = ToolRegistry::new();
    let spec = registry.get_spec(&ToolName::LargeContextFetch);
    
    assert!(spec.is_some());
    assert_eq!(spec.unwrap().name, ToolName::LargeContextFetch);
}

#[tokio::test]
async fn test_integration_large_context_fetch_live_llm() {
    // Only run if integration flag and API key(s) are provided
    if env::var("GROK_RUN_LIVE_LLM_TESTS").ok().as_deref() != Some("1") {
        eprintln!("skipping live LLM test: set GROK_RUN_LIVE_LLM_TESTS=1 to enable");
        return;
    }
    let has_key = env::var("OPENROUTER_API_KEY").is_ok() || env::var("VERCEL_AI_GATEWAY_API_KEY").is_ok();
    if !has_key {
        eprintln!("skipping live LLM test: provide OPENROUTER_API_KEY or VERCEL_AI_GATEWAY_API_KEY");
        return;
    }

    // Ensure we don't override the base URL in live run
    env::remove_var("GROK_LLM_BASE_URL");

    // Prepare a small temp project
    let dir = tempdir().unwrap();
    let base = dir.path();
    fs::write(base.join("noop.rs"), "pub fn noop() {}\n").unwrap();
    fs::write(base.join("add.rs"), "pub fn add(a: i32, b: i32) -> i32 { a + b }\n").unwrap();
    fs::write(base.join("sub.rs"), "pub fn sub(a: i32, b: i32) -> i32 { a - b }\n").unwrap();

    let bus = EventBus::new();
    let executor = LlmExecutor::new(bus.sender(), 2 * 1024 * 1024);

    let args = json!({
        "user_query": "Find the Rust file that implements a function to add two integers.",
        "base_path": base.to_string_lossy().to_string(),
        "max_files": 10
    });

    let result = executor
        .execute_large_context_fetch_with_result("live-e2e".to_string(), args)
        .await
        .expect("expected live LLM call to succeed");

    let relevant = result
        .get("relevant_files")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    assert!(!relevant.is_empty(), "expected at least one relevant file from live LLM");

    // Validate that at least one relevant file appears to contain the add implementation
    let has_add = relevant.iter().any(|f| {
        let path_match = f.get("path").and_then(|p| p.as_str()).map(|p| p.ends_with("add.rs")).unwrap_or(false);
        let content_match = f.get("contents").and_then(|c| c.as_str()).map(|c| c.contains("-> i32 { a + b }")).unwrap_or(false);
        path_match || content_match
    });
    assert!(has_add, "live LLM did not select the add function file; response: {:?}", relevant);
}
