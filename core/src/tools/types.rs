use serde::{Deserialize, Serialize};
use std::ops::Range;

// Filesystem tool types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsReadArgs {
    pub path: String,
    pub range: Option<Range<u64>>,
    pub encoding: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsReadResult {
    pub contents: String,
    pub encoding: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsSearchArgs {
    pub query: String,
    pub globs: Option<Vec<String>>,
    pub max_results: Option<u32>,
    pub regex: bool,
    pub case_insensitive: bool,
    pub multiline: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchLine {
    pub ln: u64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchMatch {
    pub path: String,
    pub lines: Vec<SearchLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsSearchResult {
    pub matches: Vec<SearchMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsWriteArgs {
    pub path: String,
    pub contents: String,
    #[serde(default = "default_create_if_missing")]
    pub create_if_missing: bool,
    #[serde(default)]
    pub overwrite: bool,
}

fn default_create_if_missing() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsWriteResult {
    pub bytes_written: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsApplyPatchArgs {
    pub unified_diff: String,
    pub dry_run: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsApplyPatchResult {
    pub success: bool,
    pub rejected_hunks: Option<Vec<String>>,
    pub summary: String,
}

// File finding tool types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsFindArgs {
    pub pattern: String,
    pub base_path: Option<String>,
    pub fuzzy: Option<bool>,
    pub case_sensitive: Option<bool>,
    pub file_type: Option<String>, // "file", "dir", "both"
    pub max_results: Option<u32>,
    pub ignore_patterns: Option<Vec<String>>, // gitignore-style patterns
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMatch {
    pub path: String,
    pub score: Option<f64>, // relevance score for fuzzy matching
    pub match_type: String, // "exact", "fuzzy", "partial"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsFindResult {
    pub matches: Vec<FileMatch>,
    pub search_time_ms: u64,
}

// Code analysis tool types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSymbolsArgs {
    pub path: String,
    pub symbol_types: Option<Vec<String>>, // "functions", "classes", "variables", etc.
    pub language: Option<String>, // auto-detect if not specified
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSymbol {
    pub name: String,
    pub symbol_type: String,
    pub line_start: u32,
    pub line_end: u32,
    pub scope: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeSymbolsResult {
    pub symbols: Vec<CodeSymbol>,
    pub language: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeFile {
    pub path: String,
    pub contents: String,
    pub language: Option<String>,
    pub size_bytes: u64,
    pub truncated: bool,
}

// Shell execution tool types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellExecArgs {
    pub command: Vec<String>,
    pub cwd: Option<String>,
    pub env: Option<Vec<(String, String)>>,
    pub timeout_ms: Option<u64>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellExecResult {
    pub exit_code: i32,
    pub duration_ms: u64,
    pub stdout: String,
    pub stderr: String,
}

// Large context fetch tool types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeContextFetchArgs {
    pub user_query: String,
    pub base_path: Option<String>,
    pub max_files: Option<u32>,
    pub include_extensions: Option<Vec<String>>,
    pub exclude_patterns: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LargeContextFetchResult {
    pub relevant_files: Vec<CodeFile>,
    pub llm_reasoning: String,
    pub total_files_analyzed: u32,
    pub total_files_returned: u32,
    pub execution_time_ms: u64,
}
