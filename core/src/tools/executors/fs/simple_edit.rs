use crate::tools::types::SimpleEditOp;
use std::collections::{BTreeMap, BTreeSet};
use std::io::ErrorKind;
use std::path::Path;

pub(crate) struct PlannedFile {
    original: Option<String>,
    current: Option<String>,
}

impl PlannedFile {
    fn existing(content: String) -> Self {
        Self { original: Some(content.clone()), current: Some(content) }
    }

    fn new_missing() -> Self {
        Self { original: None, current: None }
    }
}

pub(crate) struct SimpleEditPlanner {
    dry_run: bool,
    files: BTreeMap<String, PlannedFile>,
    renames: Vec<(String, String, bool)>,
    created: BTreeSet<String>,
    modified: BTreeSet<String>,
    deleted: BTreeSet<String>,
    descriptions: Vec<String>,
    bytes_added: u64,
    bytes_removed: u64,
}

impl SimpleEditPlanner {
    pub(crate) fn new(dry_run: bool) -> Self {
        Self {
            dry_run,
            files: BTreeMap::new(),
            renames: Vec::new(),
            created: BTreeSet::new(),
            modified: BTreeSet::new(),
            deleted: BTreeSet::new(),
            descriptions: Vec::new(),
            bytes_added: 0,
            bytes_removed: 0,
        }
    }

    pub(crate) async fn apply_op(&mut self, op: &SimpleEditOp) -> Result<(), String> {
        match op {
            SimpleEditOp::SetFile { path, contents } => {
                self.ensure_entry_allow_new(path).await?;
                let normalized = normalize_newlines(contents);
                self.set_current(path, normalized)?;
                self.descriptions.push(format!("set_file {}", path));
            }
            SimpleEditOp::ReplaceOnce { path, find, replace } => {
                self.ensure_entry(path).await?;
                let current = self.current_string(path)?;
                let needle = normalize_newlines(find);
                let replacement = normalize_newlines(replace);
                let idx = exactly_once(&current, &needle)?;
                let mut new_content = current.clone();
                new_content.replace_range(idx..idx + needle.len(), &replacement);
                self.set_current(path, new_content)?;
                self.descriptions.push(format!("replace_once {}", path));
            }
            SimpleEditOp::InsertBefore { path, anchor, insert } => {
                self.ensure_entry(path).await?;
                let current = self.current_string(path)?;
                let anchor_text = normalize_newlines(anchor);
                let insertion = normalize_newlines(insert);
                let idx = exactly_once(&current, &anchor_text)?;
                let mut new_content = current.clone();
                new_content.insert_str(idx, &insertion);
                self.set_current(path, new_content)?;
                self.descriptions.push(format!("insert_before {}", path));
            }
            SimpleEditOp::InsertAfter { path, anchor, insert } => {
                self.ensure_entry(path).await?;
                let current = self.current_string(path)?;
                let anchor_text = normalize_newlines(anchor);
                let insertion = normalize_newlines(insert);
                let idx = exactly_once(&current, &anchor_text)?;
                let mut new_content = current.clone();
                new_content.insert_str(idx + anchor_text.len(), &insertion);
                self.set_current(path, new_content)?;
                self.descriptions.push(format!("insert_after {}", path));
            }
            SimpleEditOp::DeleteFile { path } => {
                self.ensure_entry(path).await?;
                self.delete_current(path)?;
                self.descriptions.push(format!("delete_file {}", path));
            }
            SimpleEditOp::RenameFile { path, to } => {
                if path == to {
                    return Err("Source and destination paths are the same".to_string());
                }
                self.ensure_entry(path).await?;
                if self.files.get(path).and_then(|e| e.current.as_ref()).is_none() {
                    return Err(format!("File not found: {}", path));
                }
                if let Some(existing) = self.files.get(to) {
                    if existing.current.is_some() {
                        return Err(format!("Target already exists: {}", to));
                    }
                    self.files.remove(to);
                    self.created.remove(to);
                    self.modified.remove(to);
                    self.deleted.remove(to);
                } else if path_exists_on_disk(to).await? {
                    return Err(format!("Target already exists: {}", to));
                }

                let entry = self.files.remove(path).ok_or_else(|| format!("File state missing: {}", path))?;
                if entry.current.is_none() {
                    return Err(format!("File not found: {}", path));
                }
                let should_rename = entry.original.is_some();
                let to_owned = to.to_string();
                self.files.insert(to_owned.clone(), entry);
                self.reassign_path(path, &to_owned);
                self.renames.push((path.to_string(), to_owned.clone(), should_rename));
                self.descriptions.push(format!("rename_file {} -> {}", path, to));
            }
        }

        Ok(())
    }

    pub(crate) async fn finish(self) -> Result<String, String> {
        if !self.dry_run {
            self.commit().await?;
        }
        Ok(self.build_summary())
    }

    async fn ensure_entry(&mut self, path: &str) -> Result<(), String> {
        if self.files.contains_key(path) {
            return Ok(());
        }
        if let Some(content) = read_file_normalized(path).await? {
            self.files.insert(path.to_string(), PlannedFile::existing(content));
            Ok(())
        } else {
            Err(format!("File not found: {}", path))
        }
    }

    async fn ensure_entry_allow_new(&mut self, path: &str) -> Result<(), String> {
        if self.files.contains_key(path) {
            return Ok(());
        }
        if let Some(content) = read_file_normalized(path).await? {
            self.files.insert(path.to_string(), PlannedFile::existing(content));
        } else {
            self.files.insert(path.to_string(), PlannedFile::new_missing());
        }
        Ok(())
    }

    fn current_string(&self, path: &str) -> Result<String, String> {
        let entry = self.files.get(path).ok_or_else(|| format!("File state missing: {}", path))?;
        if let Some(current) = &entry.current {
            Ok(current.clone())
        } else {
            Err(format!("File has been deleted: {}", path))
        }
    }

    fn set_current(&mut self, path: &str, new_content: String) -> Result<(), String> {
        let (prev_len, original_is_none) = {
            let entry = self.files.get_mut(path).ok_or_else(|| format!("File state missing: {}", path))?;
            let prev_len = entry.current.as_ref().map(|s| s.len()).unwrap_or(0) as i64;
            entry.current = Some(new_content);
            (prev_len, entry.original.is_none())
        };
        let new_len = self
            .files
            .get(path)
            .and_then(|entry| entry.current.as_ref())
            .map(|s| s.len())
            .unwrap_or(0) as i64;
        self.record_delta(new_len - prev_len);
        if original_is_none {
            self.mark_created(path);
        } else {
            self.mark_modified(path);
        }
        Ok(())
    }

    fn delete_current(&mut self, path: &str) -> Result<(), String> {
        let (prev_len, original_is_some) = {
            let entry = self.files.get_mut(path).ok_or_else(|| format!("File state missing: {}", path))?;
            if entry.current.is_none() {
                return Err(format!("File already deleted: {}", path));
            }
            let prev_len = entry.current.as_ref().map(|s| s.len()).unwrap_or(0) as i64;
            entry.current = None;
            (prev_len, entry.original.is_some())
        };
        self.record_delta(-prev_len);
        if original_is_some {
            self.mark_deleted(path);
        } else {
            self.created.remove(path);
            self.modified.remove(path);
        }
        Ok(())
    }

    fn mark_created(&mut self, path: &str) {
        self.created.insert(path.to_string());
        self.modified.remove(path);
        self.deleted.remove(path);
    }

    fn mark_modified(&mut self, path: &str) {
        if !self.created.contains(path) {
            self.modified.insert(path.to_string());
        }
        self.deleted.remove(path);
    }

    fn mark_deleted(&mut self, path: &str) {
        self.created.remove(path);
        self.modified.remove(path);
        self.deleted.insert(path.to_string());
    }

    fn reassign_path(&mut self, from: &str, to: &str) {
        if self.created.remove(from) {
            self.created.insert(to.to_string());
        }
        if self.modified.remove(from) {
            self.modified.insert(to.to_string());
        }
        if self.deleted.remove(from) {
            self.deleted.insert(to.to_string());
        }
    }

    fn record_delta(&mut self, delta: i64) {
        if delta > 0 {
            self.bytes_added += delta as u64;
        } else if delta < 0 {
            self.bytes_removed += (-delta) as u64;
        }
    }

    async fn commit(&self) -> Result<(), String> {
        for (from, to, should_rename) in &self.renames {
            if !should_rename || from == to {
                continue;
            }
            if let Some(parent) = Path::new(to).parent() {
                if !parent.as_os_str().is_empty() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| format!("Failed to create parent directories for {}: {}", to, e))?;
                }
            }
            tokio::fs::rename(from, to)
                .await
                .map_err(|e| format!("Failed to rename {} to {}: {}", from, to, e))?;
        }

        for (path, entry) in &self.files {
            match &entry.current {
                Some(content) => {
                    if entry.original.is_none() || entry.original.as_ref() != entry.current.as_ref() {
                        if let Some(parent) = Path::new(path).parent() {
                            if !parent.as_os_str().is_empty() {
                                tokio::fs::create_dir_all(parent)
                                    .await
                                    .map_err(|e| format!("Failed to create parent directories for {}: {}", path, e))?;
                            }
                        }
                        tokio::fs::write(path, content)
                            .await
                            .map_err(|e| format!("Failed to write file {}: {}", path, e))?;
                    }
                }
                None => {
                    if entry.original.is_some() {
                        match tokio::fs::remove_file(path).await {
                            Ok(_) => {}
                            Err(e) if e.kind() == ErrorKind::NotFound => {}
                            Err(e) => return Err(format!("Failed to delete file {}: {}", path, e)),
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn build_summary(&self) -> String {
        let mut lines = Vec::new();
        if self.dry_run {
            lines.push("Dry run: no changes were written.".to_string());
        } else {
            lines.push("Edits applied successfully.".to_string());
        }

        if !self.created.is_empty() {
            lines.push(format!("Created files: {}", self.created.iter().cloned().collect::<Vec<_>>().join(", ")));
        }
        if !self.modified.is_empty() {
            lines.push(format!("Modified files: {}", self.modified.iter().cloned().collect::<Vec<_>>().join(", ")));
        }
        if !self.deleted.is_empty() {
            lines.push(format!("Deleted files: {}", self.deleted.iter().cloned().collect::<Vec<_>>().join(", ")));
        }
        if !self.renames.is_empty() {
            lines.push("Renamed files:".to_string());
            for (from, to, _) in &self.renames {
                lines.push(format!("  {} -> {}", from, to));
            }
        }
        if !self.descriptions.is_empty() {
            lines.push("Operations:".to_string());
            for desc in &self.descriptions {
                lines.push(format!("  - {}", desc));
            }
        }
        lines.push(format!("Bytes added: {}", self.bytes_added));
        lines.push(format!("Bytes removed: {}", self.bytes_removed));
        lines.join("\n")
    }
}

async fn read_file_normalized(path: &str) -> Result<Option<String>, String> {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes).to_string();
            Ok(Some(normalize_newlines(&text)))
        }
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Failed to read file {}: {}", path, e)),
    }
}

async fn path_exists_on_disk(path: &str) -> Result<bool, String> {
    match tokio::fs::metadata(path).await {
        Ok(_) => Ok(true),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
        Err(e) => Err(format!("Failed to inspect {}: {}", path, e)),
    }
}

pub(crate) fn normalize_newlines(text: &str) -> String {
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.to_string()
    }
}

fn exactly_once(haystack: &str, needle: &str) -> Result<usize, String> {
    let mut matches = haystack.match_indices(needle);
    let first = matches.next().ok_or_else(|| "anchor not found".to_string())?;
    if matches.next().is_some() {
        return Err("anchor ambiguous (found >1)".to_string());
    }
    Ok(first.0)
}
