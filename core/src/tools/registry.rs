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
                    "create_if_missing": { "type": "boolean", "default": true, "description": "Create file and parent directories if they don't exist (default: true)" },
                    "overwrite": { "type": "boolean", "default": false, "description": "Overwrite existing file (default: false)" }
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

        // fs.find
        self.specs.insert(ToolName::FsFind, ToolSpec {
            name: ToolName::FsFind,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": { "type": "string", "description": "File or directory name pattern to search for" },
                    "base_path": { "type": "string", "description": "Base directory to search from (default: current directory)" },
                    "fuzzy": { "type": "boolean", "description": "Enable fuzzy matching (default: true)" },
                    "case_sensitive": { "type": "boolean", "description": "Case sensitive search (default: false)" },
                    "file_type": { 
                        "type": "string", 
                        "enum": ["file", "dir", "both"],
                        "description": "Type of items to find (default: both)" 
                    },
                    "max_results": { "type": "integer", "minimum": 1, "description": "Maximum number of results (default: 50)" },
                    "ignore_patterns": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Gitignore-style patterns to exclude from search"
                    }
                },
                "required": ["pattern"]
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
                                "score": { "type": "number" },
                                "match_type": { "type": "string" }
                            }
                        }
                    },
                    "search_time_ms": { "type": "integer" }
                },
                "required": ["matches", "search_time_ms"]
            }),
            streaming: false,
            side_effects: false,
            needs_approval: false,
            timeout_ms: Some(10000),
        });

        // fs.read_all_code
        self.specs.insert(ToolName::FsReadAllCode, ToolSpec {
            name: ToolName::FsReadAllCode,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "base_path": { "type": "string", "description": "Base directory to search from (default: current directory)" },
                    "max_files": { "type": "integer", "minimum": 1, "description": "Maximum number of files to read (default: 100)" },
                    "exclude_patterns": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Gitignore-style patterns to exclude from search (default: common ignore patterns)"
                    },
                    "include_extensions": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "File extensions to include (default: common code file extensions)"
                    }
                }
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "files": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "contents": { "type": "string" },
                                "language": { "type": "string" },
                                "size_bytes": { "type": "integer" },
                                "truncated": { "type": "boolean" }
                            }
                        }
                    },
                    "total_files_found": { "type": "integer" },
                    "total_files_read": { "type": "integer" },
                    "total_size_bytes": { "type": "integer" },
                    "search_time_ms": { "type": "integer" }
                },
                "required": ["files", "total_files_found", "total_files_read", "total_size_bytes", "search_time_ms"]
            }),
            streaming: false,
            side_effects: false,
            needs_approval: false,
            timeout_ms: Some(30000),
        });

        // code.symbols
        self.specs.insert(ToolName::CodeSymbols, ToolSpec {
            name: ToolName::CodeSymbols,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to analyze for symbols" },
                    "symbol_types": {
                        "type": "array",
                        "items": { 
                            "type": "string",
                            "enum": ["functions", "classes", "structs", "enums", "traits", "modules", "variables", "constants", "types"]
                        },
                        "description": "Types of symbols to extract (default: all)"
                    },
                    "language": { "type": "string", "description": "Programming language (auto-detected if not specified)" }
                },
                "required": ["path"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "symbols": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": { "type": "string" },
                                "symbol_type": { "type": "string" },
                                "line_start": { "type": "integer" },
                                "line_end": { "type": "integer" },
                                "scope": { "type": "string" },
                                "visibility": { "type": "string" }
                            }
                        }
                    },
                    "language": { "type": "string" }
                },
                "required": ["symbols", "language"]
            }),
            streaming: false,
            side_effects: false,
            needs_approval: false,
            timeout_ms: Some(5000),
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
                    "duration_ms": { "type": "integer" },
                    "stdout": { "type": "string" },
                    "stderr": { "type": "string" }
                },
                "required": ["exit_code", "duration_ms", "stdout", "stderr"]
            }),
            streaming: true,
            side_effects: true,
            needs_approval: true,
            timeout_ms: Some(30000),
        });

        // large_context_fetch
        self.specs.insert(ToolName::LargeContextFetch, ToolSpec {
            name: ToolName::LargeContextFetch,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "user_query": { 
                        "type": "string", 
                        "description": "The user's query to find relevant code context for" 
                    },
                    "base_path": { 
                        "type": "string", 
                        "description": "Base directory to search from (default: current directory)" 
                    },
                    "max_files": { 
                        "type": "integer", 
                        "minimum": 1, 
                        "maximum": 500,
                        "description": "Maximum number of files to analyze (default: 200)" 
                    },
                    "include_extensions": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "File extensions to include (default: common code file extensions)"
                    },
                    "exclude_patterns": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Gitignore-style patterns to exclude from search (default: common ignore patterns)"
                    }
                },
                "required": ["user_query"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "relevant_files": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "path": { "type": "string" },
                                "contents": { "type": "string" },
                                "language": { "type": "string" },
                                "size_bytes": { "type": "integer" },
                                "truncated": { "type": "boolean" }
                            }
                        }
                    },
                    "llm_reasoning": { "type": "string" },
                    "total_files_analyzed": { "type": "integer" },
                    "total_files_returned": { "type": "integer" },
                    "execution_time_ms": { "type": "integer" }
                },
                "required": ["relevant_files", "llm_reasoning", "total_files_analyzed", "total_files_returned", "execution_time_ms"]
            }),
            streaming: false,
            side_effects: false,
            needs_approval: false,
            timeout_ms: Some(60000), // 60 seconds for LLM call
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
