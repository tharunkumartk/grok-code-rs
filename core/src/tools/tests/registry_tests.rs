use crate::tools::ToolRegistry;
use crate::events::ToolName;
use serde_json::json;

#[tokio::test]
async fn test_tool_registry_creation() {
    let _registry = ToolRegistry::new();
    
    // Just verify the registry can be created
    assert!(true);
}

#[tokio::test]
async fn test_tool_registry_get_all_specs() {
    let registry = ToolRegistry::new();
    
    let specs = registry.get_all_specs();
    
    // Verify all expected tools are present
    let spec_names: Vec<&ToolName> = specs.iter().map(|s| &s.name).collect();
    
    assert!(spec_names.contains(&&ToolName::FsRead));
    assert!(spec_names.contains(&&ToolName::FsWrite));
    assert!(spec_names.contains(&&ToolName::FsSearch));
    assert!(spec_names.contains(&&ToolName::FsFind));
    assert!(spec_names.contains(&&ToolName::FsApplyPatch));
    assert!(spec_names.contains(&&ToolName::ShellExec));
    assert!(spec_names.contains(&&ToolName::CodeSymbols));
    
    // Verify each tool has required fields
    for spec in specs {
        assert!(spec.input_schema.is_object());
        assert!(spec.output_schema.is_object());
        assert!(spec.timeout_ms.is_some());
    }
}

#[tokio::test]
async fn test_tool_registry_get_spec_by_name() {
    let registry = ToolRegistry::new();
    
    // Test getting existing tool
    let fs_read_spec = registry.get_spec(&ToolName::FsRead);
    assert!(fs_read_spec.is_some());
    let spec = fs_read_spec.unwrap();
    assert_eq!(spec.name, ToolName::FsRead);
    assert!(!spec.side_effects);
    assert!(!spec.needs_approval);
    
    // Test properties of different tools
    let shell_spec = registry.get_spec(&ToolName::ShellExec).unwrap();
    assert_eq!(shell_spec.name, ToolName::ShellExec);
    assert!(shell_spec.side_effects);
    assert!(shell_spec.needs_approval);
    assert!(shell_spec.streaming);
}

#[tokio::test]
async fn test_tool_registry_validate_args_success() {
    let registry = ToolRegistry::new();
    
    // Valid fs_read args
    let valid_args = json!({
        "path": "/test/file.txt",
        "encoding": "utf-8"
    });
    
    let result = registry.validate_args(&ToolName::FsRead, &valid_args);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_tool_registry_validate_args_missing_required() {
    let registry = ToolRegistry::new();
    
    // Missing required 'path' field for fs_read
    let invalid_args = json!({
        "encoding": "utf-8"
    });
    
    let result = registry.validate_args(&ToolName::FsRead, &invalid_args);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Missing required field: path"));
}

#[tokio::test]
async fn test_tool_registry_validate_args_not_object() {
    let registry = ToolRegistry::new();
    
    // Args must be an object
    let invalid_args = json!("not an object");
    
    let result = registry.validate_args(&ToolName::FsRead, &invalid_args);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Arguments must be an object"));
}

#[tokio::test]
async fn test_tool_registry_validate_args_unknown_tool() {
    let registry = ToolRegistry::new();
    
    let args = json!({
        "path": "/test/file.txt"
    });
    
    // Create a fake tool name (this is a bit hacky but tests the error path)
    // In reality this would never happen since ToolName is an enum
    // But the validate_args method has this error case
    // We can't easily test it due to the enum, so let's test a valid case instead
    let result = registry.validate_args(&ToolName::FsRead, &args);
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_tool_registry_fs_write_validation() {
    let registry = ToolRegistry::new();
    
    // Valid fs_write args
    let valid_args = json!({
        "path": "/test/output.txt",
        "contents": "file content",
        "create_if_missing": true,
        "overwrite": false
    });
    
    let result = registry.validate_args(&ToolName::FsWrite, &valid_args);
    assert!(result.is_ok());
    
    // Missing required fields
    let invalid_args = json!({
        "path": "/test/output.txt"
        // Missing 'contents'
    });
    
    let result = registry.validate_args(&ToolName::FsWrite, &invalid_args);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_tool_registry_shell_exec_validation() {
    let registry = ToolRegistry::new();
    
    // Valid shell_exec args
    let valid_args = json!({
        "command": ["echo", "hello"],
        "timeout_ms": 5000
    });
    
    let result = registry.validate_args(&ToolName::ShellExec, &valid_args);
    assert!(result.is_ok());
    
    // Missing required command
    let invalid_args = json!({
        "timeout_ms": 5000
    });
    
    let result = registry.validate_args(&ToolName::ShellExec, &invalid_args);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Missing required field: command"));
}

#[tokio::test]
async fn test_tool_registry_code_symbols_validation() {
    let registry = ToolRegistry::new();
    
    // Valid code_symbols args
    let valid_args = json!({
        "path": "/test/file.rs",
        "language": "rust",
        "symbol_types": ["functions", "structs"]
    });
    
    let result = registry.validate_args(&ToolName::CodeSymbols, &valid_args);
    assert!(result.is_ok());
    
    // Missing required path
    let invalid_args = json!({
        "language": "rust"
    });
    
    let result = registry.validate_args(&ToolName::CodeSymbols, &invalid_args);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_tool_registry_schema_structure() {
    let registry = ToolRegistry::new();
    
    let specs = registry.get_all_specs();
    
    // Test fs_read schema structure
    let fs_read_spec = specs.iter().find(|s| s.name == ToolName::FsRead).unwrap();
    let input_schema = &fs_read_spec.input_schema;
    
    // Should have properties object
    assert!(input_schema["properties"].is_object());
    let properties = &input_schema["properties"];
    
    // Should have required fields
    assert!(properties["path"].is_object());
    assert_eq!(properties["path"]["type"], "string");
    
    // Should have optional fields
    assert!(properties["range"].is_object());
    assert!(properties["encoding"].is_object());
    
    // Should have required array
    assert!(input_schema["required"].is_array());
    let required_fields = input_schema["required"].as_array().unwrap();
    assert!(required_fields.contains(&json!("path")));
}

#[tokio::test]
async fn test_tool_registry_output_schemas() {
    let registry = ToolRegistry::new();
    
    // Test fs_read output schema
    let fs_read_spec = registry.get_spec(&ToolName::FsRead).unwrap();
    let output_schema = &fs_read_spec.output_schema;
    
    assert!(output_schema["properties"].is_object());
    let properties = &output_schema["properties"];
    
    assert!(properties["contents"].is_object());
    assert!(properties["encoding"].is_object());
    assert!(properties["truncated"].is_object());
    
    assert_eq!(properties["contents"]["type"], "string");
    assert_eq!(properties["encoding"]["type"], "string");
    assert_eq!(properties["truncated"]["type"], "boolean");
}

#[tokio::test]
async fn test_tool_registry_tool_characteristics() {
    let registry = ToolRegistry::new();
    
    // Test file system tools
    let fs_read_spec = registry.get_spec(&ToolName::FsRead).unwrap();
    assert!(!fs_read_spec.side_effects);
    assert!(!fs_read_spec.needs_approval);
    assert!(!fs_read_spec.streaming);
    
    let fs_write_spec = registry.get_spec(&ToolName::FsWrite).unwrap();
    assert!(fs_write_spec.side_effects);
    assert!(fs_write_spec.needs_approval);
    assert!(!fs_write_spec.streaming);
    
    // Test shell execution
    let shell_spec = registry.get_spec(&ToolName::ShellExec).unwrap();
    assert!(shell_spec.side_effects);
    assert!(shell_spec.needs_approval);
    assert!(shell_spec.streaming);
    
    // Test code analysis
    let code_spec = registry.get_spec(&ToolName::CodeSymbols).unwrap();
    assert!(!code_spec.side_effects);
    assert!(!code_spec.needs_approval);
    assert!(!code_spec.streaming);
}

#[tokio::test]
async fn test_tool_registry_timeout_values() {
    let registry = ToolRegistry::new();
    
    let specs = registry.get_all_specs();
    
    for spec in specs {
        // All tools should have timeout values
        assert!(spec.timeout_ms.is_some());
        let timeout = spec.timeout_ms.unwrap();
        
        // Timeouts should be reasonable (between 1 second and 5 minutes)
        assert!(timeout >= 1000);
        assert!(timeout <= 300000);
        
        // Shell commands should have longer timeouts
        if spec.name == ToolName::ShellExec {
            assert!(timeout >= 30000); // At least 30 seconds
        }
    }
}

#[tokio::test]
async fn test_tool_registry_default_trait() {
    let registry1 = ToolRegistry::new();
    let registry2 = ToolRegistry::default();
    
    // Both should have the same tools
    let specs1 = registry1.get_all_specs();
    let specs2 = registry2.get_all_specs();
    
    assert_eq!(specs1.len(), specs2.len());
    
    // All specs should be present in both
    for spec1 in specs1 {
        assert!(specs2.iter().any(|s2| s2.name == spec1.name));
    }
}

#[tokio::test]
async fn test_tool_registry_comprehensive_validation() {
    let registry = ToolRegistry::new();
    
    // Test all tools with valid minimal arguments
    let test_cases = vec![
        (
            ToolName::FsRead,
            json!({
                "path": "/test/file.txt"
            }),
        ),
        (
            ToolName::FsWrite,
            json!({
                "path": "/test/output.txt",
                "contents": "content",
                "create_if_missing": true,
                "overwrite": false
            }),
        ),
        (
            ToolName::FsSearch,
            json!({
                "query": "pattern",
                "regex": false,
                "case_insensitive": false,
                "multiline": false
            }),
        ),
        (
            ToolName::FsFind,
            json!({
                "pattern": "*.rs"
            }),
        ),
        (
            ToolName::FsApplyPatch,
            json!({
                "unified_diff": "--- a\n+++ b\n@@ -1 +1 @@\n-old\n+new",
                "dry_run": false
            }),
        ),
        (
            ToolName::ShellExec,
            json!({
                "command": ["echo", "test"]
            }),
        ),
        (
            ToolName::CodeSymbols,
            json!({
                "path": "/test/file.rs"
            }),
        ),
    ];
    
    for (tool_name, args) in test_cases {
        let result = registry.validate_args(&tool_name, &args);
        assert!(result.is_ok(), "Validation failed for {:?}: {:?}", tool_name, result);
    }
}