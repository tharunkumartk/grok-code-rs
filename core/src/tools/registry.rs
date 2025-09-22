use crate::events::{ToolName, ToolSpec};
use serde_json::{json, Value};
use std::collections::HashMap;

/// Registry for managing available tools and their specifications
pub struct ToolRegistry {
    specs: HashMap<ToolName, ToolSpec>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            specs: HashMap::new(),
        };
        
        registry.register_builtin_tools();
        registry
    }

    /// Register all built-in tools
    fn register_builtin_tools(&mut self) {
        // fs.read
        self.specs.insert(ToolName::FsRead, ToolSpec {
            name: ToolName::FsRead,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to read" },
                    "range": {
                        "type": "object",
                        "properties": {
                            "start": { "type": "integer", "minimum": 0 },
                            "end": { "type": "integer", "minimum": 0 }
                        },
                        "description": "Optional byte range to read"
                    },
                    "encoding": { "type": "string", "description": "File encoding (default: utf-8)" }
                },
                "required": ["path"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "contents": { "type": "string" },
                    "encoding": { "type": "string" },
                    "truncated": { "type": "boolean" }
                },
                "required": ["contents", "encoding", "truncated"]
            }),
            streaming: false,
            side_effects: false,
            needs_approval: false,
            timeout_ms: Some(5000),
        });

        // fs.search
        self.specs.insert(ToolName::FsSearch, ToolSpec {
            name: ToolName::FsSearch,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "globs": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "File patterns to search"
                    },
                    "max_results": { "type": "integer", "minimum": 1, "description": "Maximum results" },
                    "regex": { "type": "boolean", "description": "Use regex search" },
                    "case_insensitive": { "type": "boolean", "description": "Case insensitive search" },
                    "multiline": { "type": "boolean", "description": "Multiline search" }
                },
                "required": ["query"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "matches": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "lines": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "ln": { "type": "integer" },
                                            "text": { "type": "string" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }),
            streaming: false,
            side_effects: false,
            needs_approval: false,
            timeout_ms: Some(10000),
        });

        // fs.write
        self.specs.insert(ToolName::FsWrite, ToolSpec {
            name: ToolName::FsWrite,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to write" },
                    "contents": { "type": "string", "description": "File contents" },
                    "create_if_missing": { "type": "boolean", "description": "Create file if it doesn't exist" },
                    "overwrite": { "type": "boolean", "description": "Overwrite existing file" }
                },
                "required": ["path", "contents"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "bytes_written": { "type": "integer" }
                },
                "required": ["bytes_written"]
            }),
            streaming: false,
            side_effects: true,
            needs_approval: true,
            timeout_ms: Some(5000),
        });

        // fs.apply_patch
        self.specs.insert(ToolName::FsApplyPatch, ToolSpec {
            name: ToolName::FsApplyPatch,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "unified_diff": { "type": "string", "description": "Unified diff to apply" },
                    "dry_run": { "type": "boolean", "description": "Dry run without applying changes" }
                },
                "required": ["unified_diff"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "success": { "type": "boolean" },
                    "rejected_hunks": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "summary": { "type": "string" }
                },
                "required": ["success", "summary"]
            }),
            streaming: false,
            side_effects: true,
            needs_approval: true,
            timeout_ms: Some(10000),
        });

        // shell.exec
        self.specs.insert(ToolName::ShellExec, ToolSpec {
            name: ToolName::ShellExec,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Command and arguments"
                    },
                    "cwd": { "type": "string", "description": "Working directory" },
                    "env": {
                        "type": "array",
                        "items": {
                            "type": "array",
                            "items": { "type": "string" },
                            "minItems": 2,
                            "maxItems": 2
                        },
                        "description": "Environment variables"
                    },
                    "timeout_ms": { "type": "integer", "description": "Timeout in milliseconds" },
                    "with_escalated_permissions": { "type": "boolean", "description": "Run with elevated permissions" },
                    "justification": { "type": "string", "description": "Justification for escalated permissions" }
                },
                "required": ["command"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "exit_code": { "type": "integer" },
                    "duration_ms": { "type": "integer" }
                },
                "required": ["exit_code", "duration_ms"]
            }),
            streaming: true,
            side_effects: true,
            needs_approval: true,
            timeout_ms: Some(30000),
        });
    }

    /// Get all tool specifications
    pub fn get_all_specs(&self) -> Vec<&ToolSpec> {
        self.specs.values().collect()
    }

    /// Get specification for a specific tool
    pub fn get_spec(&self, tool: &ToolName) -> Option<&ToolSpec> {
        self.specs.get(tool)
    }

    /// Validate arguments against tool schema
    pub fn validate_args(&self, tool: &ToolName, args: &Value) -> Result<(), String> {
        let spec = self.get_spec(tool)
            .ok_or_else(|| format!("Unknown tool: {:?}", tool))?;

        // In a real implementation, you'd use a JSON schema validator
        // For now, just basic validation
        if !args.is_object() {
            return Err("Arguments must be an object".to_string());
        }

        // Basic required field validation
        let obj = args.as_object().unwrap();
        let schema = spec.input_schema.as_object().unwrap();
        
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            for req_field in required {
                if let Some(field_name) = req_field.as_str() {
                    if !obj.contains_key(field_name) {
                        return Err(format!("Missing required field: {}", field_name));
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
