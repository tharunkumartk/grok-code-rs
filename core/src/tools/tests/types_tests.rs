use crate::tools::types::*;
use serde_json::{from_value, to_value, json};

#[test]
fn test_fs_read_args_serialization() {
    let args = FsReadArgs {
        path: "/test/path.txt".to_string(),
        range: Some(10..20),
        encoding: Some("utf-8".to_string()),
    };
    
    let serialized = to_value(&args).unwrap();
    let deserialized: FsReadArgs = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.path, args.path);
    assert_eq!(deserialized.range, args.range);
    assert_eq!(deserialized.encoding, args.encoding);
}

#[test]
fn test_fs_read_args_minimal() {
    let args = FsReadArgs {
        path: "/test/path.txt".to_string(),
        range: None,
        encoding: None,
    };
    
    let serialized = to_value(&args).unwrap();
    let deserialized: FsReadArgs = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.path, args.path);
    assert!(deserialized.range.is_none());
    assert!(deserialized.encoding.is_none());
}

#[test]
fn test_fs_read_result_serialization() {
    let result = FsReadResult {
        contents: "file contents".to_string(),
        encoding: "utf-8".to_string(),
        truncated: false,
    };
    
    let serialized = to_value(&result).unwrap();
    let deserialized: FsReadResult = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.contents, result.contents);
    assert_eq!(deserialized.encoding, result.encoding);
    assert_eq!(deserialized.truncated, result.truncated);
}

#[test]
fn test_fs_search_args_serialization() {
    let args = FsSearchArgs {
        query: "search pattern".to_string(),
        globs: Some(vec!["*.rs".to_string(), "*.toml".to_string()]),
        max_results: Some(100),
        regex: true,
        case_insensitive: false,
        multiline: true,
    };
    
    let serialized = to_value(&args).unwrap();
    let deserialized: FsSearchArgs = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.query, args.query);
    assert_eq!(deserialized.globs, args.globs);
    assert_eq!(deserialized.max_results, args.max_results);
    assert_eq!(deserialized.regex, args.regex);
    assert_eq!(deserialized.case_insensitive, args.case_insensitive);
    assert_eq!(deserialized.multiline, args.multiline);
}

#[test]
fn test_fs_search_result_serialization() {
    let search_match = SearchMatch {
        path: "/test/file.rs".to_string(),
        lines: vec![
            SearchLine {
                ln: 10,
                text: "fn test() {".to_string(),
            },
            SearchLine {
                ln: 15,
                text: "    // test comment".to_string(),
            },
        ],
    };
    
    let result = FsSearchResult {
        matches: vec![search_match],
    };
    
    let serialized = to_value(&result).unwrap();
    let deserialized: FsSearchResult = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.matches.len(), 1);
    assert_eq!(deserialized.matches[0].path, "/test/file.rs");
    assert_eq!(deserialized.matches[0].lines.len(), 2);
    assert_eq!(deserialized.matches[0].lines[0].ln, 10);
    assert_eq!(deserialized.matches[0].lines[1].text, "    // test comment");
}

#[test]
fn test_fs_write_args_serialization() {
    let args = FsWriteArgs {
        path: "/test/output.txt".to_string(),
        contents: "file contents to write".to_string(),
        create_if_missing: true,
        overwrite: false,
    };
    
    let serialized = to_value(&args).unwrap();
    let deserialized: FsWriteArgs = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.path, args.path);
    assert_eq!(deserialized.contents, args.contents);
    assert_eq!(deserialized.create_if_missing, args.create_if_missing);
    assert_eq!(deserialized.overwrite, args.overwrite);
}

#[test]
fn test_fs_write_result_serialization() {
    let result = FsWriteResult {
        bytes_written: 1024,
    };
    
    let serialized = to_value(&result).unwrap();
    let deserialized: FsWriteResult = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.bytes_written, result.bytes_written);
}


#[test]
fn test_fs_apply_patch_args_serialization() {
    let args = FsApplyPatchArgs {
        dry_run: true,
        ops: vec![
            SimpleEditOp::SetFile {
                path: "file.txt".to_string(),
                contents: "hello\n".to_string(),
            },
            SimpleEditOp::ReplaceOnce {
                path: "file.txt".to_string(),
                find: "hello\n".to_string(),
                replace: "world\n".to_string(),
            },
        ],
    };

    let serialized = to_value(&args).unwrap();
    let deserialized: FsApplyPatchArgs = from_value(serialized).unwrap();

    assert_eq!(deserialized.dry_run, args.dry_run);
    assert_eq!(deserialized.ops.len(), args.ops.len());
}

#[test]
fn test_fs_apply_patch_result_serialization() {
    let result = FsApplyPatchResult {
        success: true,
        rejected_hunks: None,
        summary: "Patch applied successfully".to_string(),
    };
    
    let serialized = to_value(&result).unwrap();
    let deserialized: FsApplyPatchResult = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.success, result.success);
    assert_eq!(deserialized.rejected_hunks, result.rejected_hunks);
    assert_eq!(deserialized.summary, result.summary);
}

#[test]
fn test_fs_apply_patch_result_with_errors() {
    let result = FsApplyPatchResult {
        success: false,
        rejected_hunks: Some(vec![
            "Hunk 1 failed to apply".to_string(),
            "Hunk 3 context mismatch".to_string(),
        ]),
        summary: "Patch failed with 2 rejected hunks".to_string(),
    };
    
    let serialized = to_value(&result).unwrap();
    let deserialized: FsApplyPatchResult = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.success, result.success);
    assert_eq!(deserialized.rejected_hunks.as_ref().unwrap().len(), 2);
    assert_eq!(deserialized.summary, result.summary);
}

#[test]
fn test_fs_find_args_serialization() {
    let args = FsFindArgs {
        pattern: "*.rs".to_string(),
        base_path: Some("/project/src".to_string()),
        fuzzy: Some(true),
        case_sensitive: Some(false),
        file_type: Some("file".to_string()),
        max_results: Some(50),
        ignore_patterns: Some(vec![
            "target/".to_string(),
            "*.tmp".to_string(),
        ]),
    };
    
    let serialized = to_value(&args).unwrap();
    let deserialized: FsFindArgs = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.pattern, args.pattern);
    assert_eq!(deserialized.base_path, args.base_path);
    assert_eq!(deserialized.fuzzy, args.fuzzy);
    assert_eq!(deserialized.case_sensitive, args.case_sensitive);
    assert_eq!(deserialized.file_type, args.file_type);
    assert_eq!(deserialized.max_results, args.max_results);
    assert_eq!(deserialized.ignore_patterns, args.ignore_patterns);
}

#[test]
fn test_fs_find_result_serialization() {
    let file_matches = vec![
        FileMatch {
            path: "/project/src/main.rs".to_string(),
            score: Some(0.95),
            match_type: "exact".to_string(),
        },
        FileMatch {
            path: "/project/src/lib.rs".to_string(),
            score: Some(0.87),
            match_type: "fuzzy".to_string(),
        },
    ];
    
    let result = FsFindResult {
        matches: file_matches,
        search_time_ms: 42,
    };
    
    let serialized = to_value(&result).unwrap();
    let deserialized: FsFindResult = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.matches.len(), 2);
    assert_eq!(deserialized.matches[0].path, "/project/src/main.rs");
    assert_eq!(deserialized.matches[0].score, Some(0.95));
    assert_eq!(deserialized.matches[1].match_type, "fuzzy");
    assert_eq!(deserialized.search_time_ms, 42);
}

#[test]
fn test_code_symbols_args_serialization() {
    let args = CodeSymbolsArgs {
        path: "/project/src/main.rs".to_string(),
        symbol_types: Some(vec![
            "functions".to_string(),
            "structs".to_string(),
            "enums".to_string(),
        ]),
        language: Some("rust".to_string()),
    };
    
    let serialized = to_value(&args).unwrap();
    let deserialized: CodeSymbolsArgs = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.path, args.path);
    assert_eq!(deserialized.symbol_types, args.symbol_types);
    assert_eq!(deserialized.language, args.language);
}

#[test]
fn test_code_symbols_result_serialization() {
    let symbols = vec![
        CodeSymbol {
            name: "main".to_string(),
            symbol_type: "function".to_string(),
            line_start: 1,
            line_end: 5,
            scope: None,
            visibility: Some("public".to_string()),
        },
        CodeSymbol {
            name: "MyStruct".to_string(),
            symbol_type: "struct".to_string(),
            line_start: 7,
            line_end: 12,
            scope: Some("crate".to_string()),
            visibility: Some("public".to_string()),
        },
    ];
    
    let result = CodeSymbolsResult {
        symbols,
        language: "rust".to_string(),
    };
    
    let serialized = to_value(&result).unwrap();
    let deserialized: CodeSymbolsResult = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.language, "rust");
    assert_eq!(deserialized.symbols.len(), 2);
    assert_eq!(deserialized.symbols[0].name, "main");
    assert_eq!(deserialized.symbols[0].symbol_type, "function");
    assert_eq!(deserialized.symbols[1].name, "MyStruct");
    assert_eq!(deserialized.symbols[1].visibility, Some("public".to_string()));
}

#[test]
fn test_shell_exec_args_serialization() {
    let args = ShellExecArgs {
        command: vec!["ls".to_string(), "-la".to_string(), "/tmp".to_string()],
        cwd: Some("/project".to_string()),
        env: Some(vec![
            ("PATH".to_string(), "/usr/bin:/bin".to_string()),
            ("DEBUG".to_string(), "1".to_string()),
        ]),
        timeout_ms: Some(30000),
        with_escalated_permissions: Some(false),
        justification: Some("Listing files for analysis".to_string()),
    };
    
    let serialized = to_value(&args).unwrap();
    let deserialized: ShellExecArgs = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.command, args.command);
    assert_eq!(deserialized.cwd, args.cwd);
    assert_eq!(deserialized.env, args.env);
    assert_eq!(deserialized.timeout_ms, args.timeout_ms);
    assert_eq!(deserialized.with_escalated_permissions, args.with_escalated_permissions);
    assert_eq!(deserialized.justification, args.justification);
}

#[test]
fn test_shell_exec_result_serialization() {
    let result = ShellExecResult {
        exit_code: 0,
        duration_ms: 1250,
        stdout: "total 42\ndrwxr-xr-x 2 user user 4096 Jan  1 12:00 .\n".to_string(),
        stderr: "".to_string(),
    };
    
    let serialized = to_value(&result).unwrap();
    let deserialized: ShellExecResult = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.exit_code, result.exit_code);
    assert_eq!(deserialized.duration_ms, result.duration_ms);
    assert_eq!(deserialized.stdout, result.stdout);
    assert_eq!(deserialized.stderr, result.stderr);
}

#[test]
fn test_shell_exec_result_with_error() {
    let result = ShellExecResult {
        exit_code: 1,
        duration_ms: 500,
        stdout: "".to_string(),
        stderr: "command not found: nonexistent_command\n".to_string(),
    };
    
    let serialized = to_value(&result).unwrap();
    let deserialized: ShellExecResult = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.exit_code, 1);
    assert!(deserialized.stderr.contains("command not found"));
}

#[test]
fn test_complex_nested_structures() {
    // Test a complex search result with multiple matches and lines
    let complex_result = FsSearchResult {
        matches: vec![
            SearchMatch {
                path: "/project/src/main.rs".to_string(),
                lines: vec![
                    SearchLine { ln: 1, text: "use std::collections::HashMap;".to_string() },
                    SearchLine { ln: 15, text: "fn main() {".to_string() },
                    SearchLine { ln: 25, text: "    let mut map = HashMap::new();".to_string() },
                ],
            },
            SearchMatch {
                path: "/project/src/lib.rs".to_string(),
                lines: vec![
                    SearchLine { ln: 8, text: "pub fn create_map() -> HashMap<String, i32> {".to_string() },
                ],
            },
        ],
    };
    
    let serialized = to_value(&complex_result).unwrap();
    let deserialized: FsSearchResult = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.matches.len(), 2);
    assert_eq!(deserialized.matches[0].lines.len(), 3);
    assert_eq!(deserialized.matches[1].lines.len(), 1);
    assert!(deserialized.matches[0].lines[2].text.contains("HashMap"));
}

#[test]
fn test_optional_fields_none() {
    // Test structures with all optional fields set to None
    let minimal_find_args = FsFindArgs {
        pattern: "test".to_string(),
        base_path: None,
        fuzzy: None,
        case_sensitive: None,
        file_type: None,
        max_results: None,
        ignore_patterns: None,
    };
    
    let serialized = to_value(&minimal_find_args).unwrap();
    let deserialized: FsFindArgs = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.pattern, "test");
    assert!(deserialized.base_path.is_none());
    assert!(deserialized.fuzzy.is_none());
    assert!(deserialized.case_sensitive.is_none());
    assert!(deserialized.file_type.is_none());
    assert!(deserialized.max_results.is_none());
    assert!(deserialized.ignore_patterns.is_none());
}

#[test]
fn test_range_serialization() {
    use std::ops::Range;
    
    let range: Range<u64> = 10..50;
    let serialized = to_value(&range).unwrap();
    let deserialized: Range<u64> = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.start, 10);
    assert_eq!(deserialized.end, 50);
}

#[test]
fn test_default_values() {
    // Test that boolean fields work correctly
    let search_args = FsSearchArgs {
        query: "test".to_string(),
        globs: None,
        max_results: None,
        regex: false,
        case_insensitive: true,
        multiline: false,
    };
    
    let serialized = to_value(&search_args).unwrap();
    let deserialized: FsSearchArgs = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.regex, false);
    assert_eq!(deserialized.case_insensitive, true);
    assert_eq!(deserialized.multiline, false);
}

#[test]
fn test_empty_collections() {
    // Test with empty vectors
    let result = FsSearchResult {
        matches: vec![],
    };
    
    let serialized = to_value(&result).unwrap();
    let deserialized: FsSearchResult = from_value(serialized).unwrap();
    
    assert!(deserialized.matches.is_empty());
    
    // Test with empty command
    let shell_args = ShellExecArgs {
        command: vec![],
        cwd: None,
        env: None,
        timeout_ms: None,
        with_escalated_permissions: None,
        justification: None,
    };
    
    let serialized = to_value(&shell_args).unwrap();
    let deserialized: ShellExecArgs = from_value(serialized).unwrap();
    
    assert!(deserialized.command.is_empty());
}

#[test]
fn test_large_data_structures() {
    // Test with many symbols
    let mut symbols = Vec::new();
    for i in 0..100 {
        symbols.push(CodeSymbol {
            name: format!("function_{}", i),
            symbol_type: "function".to_string(),
            line_start: i as u32 * 10,
            line_end: i as u32 * 10 + 5,
            scope: None,
            visibility: Some("public".to_string()),
        });
    }
    
    let result = CodeSymbolsResult {
        symbols,
        language: "rust".to_string(),
    };
    
    let serialized = to_value(&result).unwrap();
    let deserialized: CodeSymbolsResult = from_value(serialized).unwrap();
    
    assert_eq!(deserialized.symbols.len(), 100);
    assert_eq!(deserialized.symbols[50].name, "function_50");
    assert_eq!(deserialized.symbols[99].line_start, 990);
}

#[test]
fn test_fs_write_args_defaults() {
    // Test with missing boolean fields - should use defaults
    let args_missing_bools = json!({
        "path": "/test/file.txt",
        "contents": "test content"
    });
    
    let args: FsWriteArgs = from_value(args_missing_bools).unwrap();
    assert_eq!(args.path, "/test/file.txt");
    assert_eq!(args.contents, "test content");
    assert_eq!(args.create_if_missing, true, "create_if_missing should default to true");
    assert_eq!(args.overwrite, false, "overwrite should default to false");
    
    // Test with explicit boolean fields
    let args_with_bools = json!({
        "path": "/test/file.txt",
        "contents": "test content",
        "create_if_missing": false,
        "overwrite": true
    });
    
    let args: FsWriteArgs = from_value(args_with_bools).unwrap();
    assert_eq!(args.create_if_missing, false);
    assert_eq!(args.overwrite, true);
}
