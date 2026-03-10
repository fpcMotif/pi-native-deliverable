#![forbid(unsafe_code)]

use pi_protocol::session::{SessionEntry, SessionEntryKind, SessionLog};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, SessionError>;

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("io: {0}")]
    Io(#[from] io::Error),
    #[error("serde: {0}")]
    Json(#[from] serde_json::Error),
    #[error("entry {0} not found")]
    MissingEntry(String),
    #[error("session path rejected: {0}")]
    InvalidPath(String),
}

#[derive(Debug, Clone)]
pub enum SessionQuery {
    All,
    BranchHead(Option<Uuid>),
    Since(Uuid),
}

#[derive(Debug)]
pub struct SessionStore {
    path: PathBuf,
    pub log: SessionLog,
    entry_by_id: HashMap<Uuid, usize>,
    children: HashMap<Uuid, Vec<Uuid>>,
    roots: Vec<Uuid>,
    head_id: Option<Uuid>,
}

impl SessionStore {
    pub fn default_session_path(workspace_root: impl AsRef<Path>) -> PathBuf {
        workspace_root.as_ref().join(".pi").join("session.jsonl")
    }

    pub fn resolve_session_path(
        requested: impl AsRef<Path>,
        workspace_root: impl AsRef<Path>,
    ) -> Result<PathBuf> {
        let workspace_root = workspace_root.as_ref();
        let requested = requested.as_ref();

        let workspace_root = workspace_root
            .canonicalize()
            .map_err(|err| SessionError::InvalidPath(err.to_string()))?;
        let candidate = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            workspace_root.join(requested)
        };
        let normalized = normalize_path(&candidate);
        if !normalized.starts_with(&workspace_root) {
            return Err(SessionError::InvalidPath(
                "session path is outside workspace".to_string(),
            ));
        }

        if let Some(parent) = normalized.parent() {
            fs::create_dir_all(parent)?;
        }
        Ok(normalized)
    }

    pub async fn new(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let entries = if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            let path_clone = path.clone();
            tokio::task::spawn_blocking(move || -> Result<Vec<SessionEntry>> {
                let mut entries = Vec::new();
                let file = std::fs::File::open(&path_clone)?;
                use std::io::BufRead;
                let mut reader = std::io::BufReader::new(file);
                let mut line = String::new();

                while reader.read_line(&mut line)? > 0 {
                    let raw = line.trim();
                    if raw.is_empty() {
                        line.clear();
                        continue;
                    }

                    match serde_json::from_str::<SessionEntry>(raw) {
                        Ok(value) => entries.push(value),
                        Err(parse_err) => {
                            let legacy =
                                serde_json::from_str::<LegacyLog>(raw).map_err(|_| parse_err)?;
                            entries.extend(legacy.entries);
                        }
                    }
                    line.clear();
                }
                Ok(entries)
            })
            .await
            .map_err(|e| SessionError::InvalidPath(e.to_string()))??
        } else {
            tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .await?;
            Vec::new()
        };

        let mut store = Self {
            path,
            log: SessionLog { entries },
            entry_by_id: HashMap::new(),
            children: HashMap::new(),
            roots: Vec::new(),
            head_id: None,
        };
        store.rebuild_index();
        Ok(store)
    }

    pub async fn load(path: impl Into<PathBuf>) -> Result<Self> {
        Self::new(path).await
    }

    pub async fn append_entry(&mut self, mut entry: SessionEntry) -> Result<Uuid> {
        if entry.entry_id == Uuid::nil() {
            entry.entry_id = Uuid::new_v4();
        }

        if let Some(parent_id) = entry.parent_id {
            if !self.entry_by_id.contains_key(&parent_id) {
                return Err(SessionError::MissingEntry(parent_id.to_string()));
            }
            self.children
                .entry(parent_id)
                .or_default()
                .push(entry.entry_id);
        } else {
            self.roots.push(entry.entry_id);
        }

        entry.schema_version = if entry.schema_version.is_empty() {
            "1.0".to_string()
        } else {
            entry.schema_version
        };
        entry.timestamp_ms = if entry.timestamp_ms == 0 {
            Self::now_ms()
        } else {
            entry.timestamp_ms
        };

        let id = entry.entry_id;
        self.log.entries.push(entry);
        self.entry_by_id.insert(id, self.log.entries.len() - 1);
        self.head_id = Some(id);

        let mut file = tokio::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&self.path)
            .await?;
        let bytes = serde_json::to_vec(&self.log.entries.last().expect("entry"))?;
        use tokio::io::AsyncWriteExt;
        file.write_all(&bytes).await?;
        file.write_all(b"\n").await?;
        Ok(id)
    }

    pub async fn append(&mut self, kind: SessionEntryKind) -> Result<Uuid> {
        let parent_id = self.head_id;
        let entry = SessionEntry::new(kind, parent_id);
        self.append_entry(entry).await
    }

    pub fn get_branch_head(&self) -> Option<Uuid> {
        self.head_id
    }

    pub async fn current_head(&self) -> Option<Uuid> {
        self.head_id
    }

    pub async fn checkout(&mut self, entry_id: Uuid) -> bool {
        if self.entry_by_id.contains_key(&entry_id) {
            self.head_id = Some(entry_id);
            true
        } else {
            false
        }
    }

    pub async fn branch_from(&mut self, entry_id: Uuid) -> Result<Uuid> {
        if !self.entry_by_id.contains_key(&entry_id) {
            return Err(SessionError::MissingEntry(entry_id.to_string()));
        }

        let fork = SessionEntry::new(
            SessionEntryKind::SessionFork {
                from_entry_id: entry_id,
                summary: Some(format!("branch from {entry_id}")),
            },
            Some(entry_id),
        );
        self.append_entry(fork).await
    }

    pub fn load_tree(&self) -> (&Vec<Uuid>, &HashMap<Uuid, Vec<Uuid>>) {
        (&self.roots, &self.children)
    }

    pub async fn compact(&mut self, path: Option<&Path>) -> Result<usize> {
        let target = path
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.path.with_extension("compact.jsonl"));

        let mut lines = String::new();
        for entry in &self.log.entries {
            lines.push_str(&canonical_json(entry)?);
            lines.push('\n');
        }

        let temp_path = target.with_extension("tmp");
        tokio::fs::write(&temp_path, lines.as_bytes()).await?;
        if tokio::fs::try_exists(&target).await.unwrap_or(false) {
            tokio::fs::remove_file(&target).await?;
        }
        tokio::fs::rename(&temp_path, &target).await?;

        if target != self.path {
            // keep canonical compact copy and continue using source path.
            tokio::fs::copy(&target, &self.path).await?;
        }

        Ok(self.log.entries.len())
    }

    pub fn to_text_summary(&self) -> String {
        format!(
            "entries={} roots={} head={:?}",
            self.log.entries.len(),
            self.roots.len(),
            self.head_id
        )
    }

    pub fn to_jsonl_string(&self) -> Result<String> {
        self.log
            .entries
            .iter()
            .map(canonical_json)
            .collect::<Result<Vec<_>>>()
            .map(|lines| lines.join("\n"))
    }

    pub fn prune_to_depth(&self, max_depth: usize) -> Vec<Uuid> {
        let mut out = Vec::new();
        let mut cursor = self.head_id;
        let mut remaining = max_depth;

        while let Some(current) = cursor {
            if remaining == 0 {
                break;
            }
            out.push(current);
            cursor = self
                .log
                .entries
                .iter()
                .find(|entry| entry.entry_id == current)
                .and_then(|entry| entry.parent_id);
            remaining -= 1;
        }

        out
    }

    pub fn reset(&mut self) -> Result<()> {
        self.log.entries.clear();
        self.entry_by_id.clear();
        self.children.clear();
        self.roots.clear();
        self.head_id = None;
        fs::write(&self.path, b"")?;
        Ok(())
    }

    pub fn rebuild_index(&mut self) {
        self.entry_by_id.clear();
        self.children.clear();
        self.roots.clear();

        let mut parents: HashSet<Uuid> = HashSet::new();
        for (index, entry) in self.log.entries.iter().enumerate() {
            if let Some(parent_id) = entry.parent_id {
                parents.insert(parent_id);
                self.children
                    .entry(parent_id)
                    .or_default()
                    .push(entry.entry_id);
            }
            self.entry_by_id.insert(entry.entry_id, index);
        }

        for entry in &self.log.entries {
            if !parents.contains(&entry.entry_id) {
                self.roots.push(entry.entry_id);
            }
        }

        self.head_id = self.log.entries.last().map(|entry| entry.entry_id);
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

fn canonical_json<T: Serialize>(value: &T) -> Result<String> {
    let value = serde_json::to_value(value)?;
    let canonical = serde_json::to_string(&canonicalize_json_value(value))?;
    Ok(canonical)
}

fn canonicalize_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut ordered = Map::new();
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort_unstable();
            for key in keys {
                if let Some(next) = map.get(&key) {
                    ordered.insert(key, canonicalize_json_value(next.clone()));
                }
            }
            Value::Object(ordered)
        }
        Value::Array(values) => {
            Value::Array(values.into_iter().map(canonicalize_json_value).collect())
        }
        other => other,
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = if path.is_absolute() {
        PathBuf::from("/")
    } else {
        PathBuf::new()
    };
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(segment) => normalized.push(segment),
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
        }
    }
    normalized
}

#[derive(Debug, Serialize, serde::Deserialize)]
struct LegacyLog {
    pub entries: Vec<SessionEntry>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[tokio::test]
    async fn test_checkout_nonexistent_entry() {
        let temp_dir = env::temp_dir();
        let session_path = temp_dir.join(format!("{}.jsonl", Uuid::new_v4()));

        // Initialize a minimal SessionStore
        let mut store = SessionStore::new(&session_path).await.unwrap();

        // Ensure head_id is initially None
        assert_eq!(store.get_branch_head(), None);

        // Attempt to checkout a random, non-existent UUID
        let random_id = Uuid::new_v4();
        let result = store.checkout(random_id).await;

        // Verify it returns false and doesn't change head_id
        assert!(!result);
        assert_eq!(store.get_branch_head(), None);

        // Clean up
        let _ = fs::remove_file(session_path);
    }
}
