use pi_protocol::session::{normalize_jsonl, SessionEntry, SessionEntryKind};
use pi_session::SessionStore;
use serde_json::json;
use std::fs;
use uuid::Uuid;

#[tokio::test]
async fn session_jsonl_roundtrip_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let session_path = tmp.path().join("session.jsonl");

    let user_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").expect("uuid");
    let assistant_id = Uuid::parse_str("00000000-0000-0000-0000-000000000002").expect("uuid");
    let usage_id = Uuid::parse_str("00000000-0000-0000-0000-000000000003").expect("uuid");

    let mut store = SessionStore::new(session_path.clone())
        .await
        .expect("new store");

    store
        .append_entry(SessionEntry {
            schema_version: "1.0".to_string(),
            entry_id: user_id,
            timestamp_ms: 1,
            kind: SessionEntryKind::UserMessage {
                text: "hello".to_string(),
            },
            parent_id: None,
            metadata: Some(json!({"source": "test"})),
        })
        .await
        .expect("append user");

    store
        .append_entry(SessionEntry {
            schema_version: "1.0".to_string(),
            entry_id: assistant_id,
            timestamp_ms: 2,
            kind: SessionEntryKind::AssistantMessage {
                text: "hi".to_string(),
            },
            parent_id: Some(user_id),
            metadata: None,
        })
        .await
        .expect("append assistant");

    let usage_payload = json!({
        "model": "gpt-5-mini",
        "provider": "openai-compatible",
        "output_tokens": 34,
        "input_tokens": 21,
        "cached_tokens": 8,
        "cost_usd": 0.0012,
    });

    store
        .append_entry(SessionEntry {
            schema_version: "1.0".to_string(),
            entry_id: usage_id,
            timestamp_ms: 3,
            kind: SessionEntryKind::SessionMetadata {
                payload: usage_payload.clone(),
            },
            parent_id: Some(assistant_id),
            metadata: Some(json!({"note": "usage aggregate"})),
        })
        .await
        .expect("append usage metadata");

    let raw_before = fs::read_to_string(&session_path).expect("read before compact");

    let reloaded = SessionStore::load(session_path.clone())
        .await
        .expect("reload store");
    let metadata_entry = reloaded
        .log
        .entries
        .iter()
        .find_map(|entry| match &entry.kind {
            SessionEntryKind::SessionMetadata { payload } => {
                Some((payload, entry.metadata.as_ref()))
            }
            _ => None,
        })
        .expect("metadata entry present");

    assert_eq!(metadata_entry.0, &usage_payload);
    assert_eq!(metadata_entry.1, Some(&json!({"note": "usage aggregate"})));

    let compacted = SessionStore::load(session_path.clone())
        .await
        .expect("reload for compact");
    let mut compacted = compacted;
    compacted.compact(None).await.expect("compact");

    let raw_after = fs::read_to_string(&session_path).expect("read after compact");
    assert_eq!(
        normalize_jsonl(&raw_before).expect("normalize before"),
        normalize_jsonl(&raw_after).expect("normalize after")
    );

    let roundtripped = SessionStore::load(session_path)
        .await
        .expect("reload after compact");
    let metadata_after = roundtripped
        .log
        .entries
        .iter()
        .find_map(|entry| match &entry.kind {
            SessionEntryKind::SessionMetadata { payload } => {
                Some((payload, entry.metadata.as_ref()))
            }
            _ => None,
        })
        .expect("metadata entry after compact");

    assert_eq!(metadata_after.0, &usage_payload);
    assert_eq!(metadata_after.1, Some(&json!({"note": "usage aggregate"})));
}
