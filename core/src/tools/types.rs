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
    pub create_if_missing: bool,
    pub overwrite: bool,
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
}
