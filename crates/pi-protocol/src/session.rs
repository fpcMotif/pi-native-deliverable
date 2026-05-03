#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::io::{self, BufRead};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionEntryKind {
    SystemPromptSet {
        text: String,
    },
    UserMessage {
        text: String,
    },
    AssistantMessage {
        text: String,
    },
    ToolCall {
        tool_name: String,
        args: Value,
    },
    ToolResult {
        tool_name: String,
        output: Value,
        success: bool,
    },
    ModelChange {
        model: String,
    },
    ThinkingLevelChange {
        level: String,
    },
    CompactionSnapshot {
        summary: String,
    },
    SessionFork {
        from_entry_id: Uuid,
        summary: Option<String>,
    },
    SessionMetadata {
        payload: Value,
    },
    ExtensionEvent {
        manifest: String,
        action: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub schema_version: String,
    pub entry_id: Uuid,
    pub timestamp_ms: u64,
    pub kind: SessionEntryKind,
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

impl SessionEntry {
    pub fn new(kind: SessionEntryKind, parent_id: Option<Uuid>) -> Self {
        Self {
            schema_version: "1.0".to_string(),
            entry_id: Uuid::new_v4(),
            timestamp_ms: now_ms(),
            kind,
            parent_id,
            metadata: None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionLog {
    pub entries: Vec<SessionEntry>,
}

impl SessionLog {
    pub fn load_jsonl(path: &Path) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = io::BufReader::new(file);
        let mut entries = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let entry = serde_json::from_str::<SessionEntry>(&line)
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;
            entries.push(entry);
        }

        Ok(Self { entries })
    }

    pub fn append_entry(&mut self, entry: SessionEntry) {
        self.entries.push(entry);
    }

    pub fn children(&self) -> HashMap<Uuid, Vec<Uuid>> {
        let mut map: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
        for entry in &self.entries {
            if let Some(parent) = entry.parent_id {
                map.entry(parent).or_default().push(entry.entry_id);
            }
        }
        map
    }

    pub fn roots(&self) -> Vec<Uuid> {
        let has_parent: HashSet<Uuid> = self
            .entries
            .iter()
            .filter_map(|entry| entry.parent_id)
            .collect();

        self.entries
            .iter()
            .filter(|entry| !has_parent.contains(&entry.entry_id))
            .map(|entry| entry.entry_id)
            .collect()
    }

    pub fn to_jsonl_string(&self) -> Result<String, serde_json::Error> {
        let mut lines = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            lines.push(serde_json::to_string(entry)?);
        }
        Ok(lines.join("\n"))
    }
}

#[derive(Debug, Clone)]
pub enum SessionQuery {
    All,
    BranchHead(Option<Uuid>),
    Since(Option<Uuid>),
}

pub fn normalize_jsonl(raw: &str) -> Result<String, serde_json::Error> {
    let mut normalized = Vec::new();

    for line in raw.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(line)?;
        normalized.push(canonicalize_json_value(value).to_string());
    }

    Ok(normalized.join("\n"))
}

fn canonicalize_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut sorted = Map::new();
            let mut keys: Vec<String> = map.keys().cloned().collect();
            keys.sort_unstable();
            for key in keys {
                sorted.insert(key.clone(), canonicalize_json_value(map[&key].clone()));
            }
            Value::Object(sorted)
        }
        Value::Array(items) => {
            Value::Array(items.into_iter().map(canonicalize_json_value).collect())
        }
        other => other,
    }
}

pub fn summarize_entries(entries: &[SessionEntry]) -> String {
    let mut out = String::new();
    let _ = writeln!(&mut out, "entries={}", entries.len());
    for (idx, entry) in entries.iter().enumerate() {
        let _ = writeln!(
            &mut out,
            "{} {} {}",
            idx,
            entry.entry_id,
            display_entry_kind(&entry.kind)
        );
    }
    out
}

fn display_entry_kind(kind: &SessionEntryKind) -> &'static str {
    match kind {
        SessionEntryKind::SystemPromptSet { .. } => "system_prompt_set",
        SessionEntryKind::UserMessage { .. } => "user_message",
        SessionEntryKind::AssistantMessage { .. } => "assistant_message",
        SessionEntryKind::ToolCall { .. } => "tool_call",
        SessionEntryKind::ToolResult { .. } => "tool_result",
        SessionEntryKind::ModelChange { .. } => "model_change",
        SessionEntryKind::ThinkingLevelChange { .. } => "thinking_level_change",
        SessionEntryKind::CompactionSnapshot { .. } => "compaction_snapshot",
        SessionEntryKind::SessionFork { .. } => "session_fork",
        SessionEntryKind::SessionMetadata { .. } => "session_metadata",
        SessionEntryKind::ExtensionEvent { .. } => "extension_event",
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_jsonl_empty() -> Result<(), serde_json::Error> {
        assert_eq!(normalize_jsonl("")?, "");
        Ok(())
    }

    #[test]
    fn test_normalize_jsonl_skip_empty_lines() -> Result<(), serde_json::Error> {
        assert_eq!(normalize_jsonl("\n   \n\n")?, "");
        Ok(())
    }

    #[test]
    fn test_normalize_jsonl_single_object() {
        let raw = r#"{"b": 2, "a": 1}"#;
        let expected = r#"{"a":1,"b":2}"#;
        assert_eq!(
            normalize_jsonl(raw).expect("Failed to normalize JSONL"),
            expected
        );
    }

    #[test]
    fn test_normalize_jsonl_nested_objects() {
        let raw = r#"{"z": {"d": 4, "c": 3}, "a": {"b": 2, "a": 1}}"#;
        let expected = r#"{"a":{"a":1,"b":2},"z":{"c":3,"d":4}}"#;
        assert_eq!(
            normalize_jsonl(raw).expect("Failed to normalize JSONL"),
            expected
        );
    }

    #[test]
    fn test_normalize_jsonl_arrays() -> Result<(), serde_json::Error> {
        let raw = r#"[{"b": 2, "a": 1}, {"d": 4, "c": 3}]"#;
        let expected = r#"[{"a":1,"b":2},{"c":3,"d":4}]"#;
        assert_eq!(normalize_jsonl(raw)?, expected);
        Ok(())
    }

    #[test]
    fn test_normalize_jsonl_multiple_lines() -> Result<(), serde_json::Error> {
        let raw = "{\"b\": 2, \"a\": 1}\n\n{\"d\": 4, \"c\": 3}";
        let expected = "{\"a\":1,\"b\":2}\n{\"c\":3,\"d\":4}";
        assert_eq!(normalize_jsonl(raw)?, expected);
        Ok(())
    }

    #[test]
    fn test_normalize_jsonl_invalid_json() {
        let raw = r#"{"a": 1"#;
        assert!(normalize_jsonl(raw).is_err());
    }

    #[test]
    fn test_load_jsonl_valid() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let file_path = dir.path().join("valid.jsonl");

        let entry1 = SessionEntry::new(
            SessionEntryKind::UserMessage {
                text: "hello".to_string(),
            },
            None,
        );
        let entry2 = SessionEntry::new(
            SessionEntryKind::AssistantMessage {
                text: "hi".to_string(),
            },
            Some(entry1.entry_id),
        );

        let mut log = SessionLog::default();
        log.append_entry(entry1.clone());
        log.append_entry(entry2.clone());

        let jsonl = log.to_jsonl_string().unwrap();
        std::fs::write(&file_path, jsonl)?;

        let loaded = SessionLog::load_jsonl(&file_path)?;
        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0].entry_id, entry1.entry_id);
        assert_eq!(loaded.entries[1].entry_id, entry2.entry_id);

        Ok(())
    }

    #[test]
    fn test_load_jsonl_empty_lines() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let file_path = dir.path().join("empty_lines.jsonl");

        let entry1 = SessionEntry::new(
            SessionEntryKind::UserMessage {
                text: "hello".to_string(),
            },
            None,
        );
        let entry_json = serde_json::to_string(&entry1).unwrap();

        let jsonl = format!(
            "

{}

",
            entry_json
        );
        std::fs::write(&file_path, jsonl)?;

        let loaded = SessionLog::load_jsonl(&file_path)?;
        assert_eq!(loaded.entries.len(), 1);
        assert_eq!(loaded.entries[0].entry_id, entry1.entry_id);

        Ok(())
    }

    #[test]
    fn test_load_jsonl_invalid_json() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let file_path = dir.path().join("invalid.jsonl");

        std::fs::write(&file_path, r#"{"kind":"system_prompt_set""#)?;

        let result = SessionLog::load_jsonl(&file_path);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);

        Ok(())
    }

    #[test]
    fn test_load_jsonl_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("missing.jsonl");

        let result = SessionLog::load_jsonl(&file_path);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::NotFound);
    }
}
