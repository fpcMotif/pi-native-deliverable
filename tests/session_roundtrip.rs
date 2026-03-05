use pi_protocol::session::{normalize_jsonl, SessionEntry, SessionEntryKind, SessionLog};
use std::fs;
use uuid::Uuid;

/// Session JSONL roundtrip: create entries programmatically, write as JSONL,
/// load via SessionLog::load_jsonl, serialize back, and verify normalized
/// JSON is identical.
#[test]
fn session_jsonl_roundtrip_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let session_path = tmp.path().join("session.jsonl");

    let id1 = Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
    let id2 = Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap();

    let entry1 = SessionEntry {
        schema_version: "1.0".to_string(),
        entry_id: id1,
        timestamp_ms: 1000,
        kind: SessionEntryKind::UserMessage {
            text: "hello".to_string(),
        },
        parent_id: None,
        metadata: None,
    };

    let entry2 = SessionEntry {
        schema_version: "1.0".to_string(),
        entry_id: id2,
        timestamp_ms: 2000,
        kind: SessionEntryKind::AssistantMessage {
            text: "hi".to_string(),
        },
        parent_id: Some(id1),
        metadata: None,
    };

    // Serialize entries to JSONL
    let line1 = serde_json::to_string(&entry1).expect("serialize entry1");
    let line2 = serde_json::to_string(&entry2).expect("serialize entry2");
    let raw_input = format!("{line1}\n{line2}\n");

    fs::write(&session_path, &raw_input).expect("write session");

    // Load back via SessionLog
    let log = SessionLog::load_jsonl(&session_path).expect("load session log");

    assert_eq!(log.entries.len(), 2, "expected 2 entries after loading");

    assert_eq!(log.entries[0].entry_id, id1);
    assert_eq!(log.entries[1].entry_id, id2);
    assert_eq!(log.entries[1].parent_id, Some(id1));

    // Verify kind discrimination
    assert!(matches!(
        log.entries[0].kind,
        SessionEntryKind::UserMessage { ref text } if text == "hello"
    ));
    assert!(matches!(
        log.entries[1].kind,
        SessionEntryKind::AssistantMessage { ref text } if text == "hi"
    ));

    // Roundtrip: serialize back and verify normalized equivalence
    let roundtrip_jsonl = log.to_jsonl_string().expect("serialize roundtrip");

    let normalized_input = normalize_jsonl(&raw_input).expect("normalize input");
    let normalized_roundtrip = normalize_jsonl(&roundtrip_jsonl).expect("normalize roundtrip");

    assert_eq!(
        normalized_input, normalized_roundtrip,
        "normalized JSONL should be identical after roundtrip"
    );
}
