use pi_protocol::session::{normalize_jsonl, SessionEntryKind, SessionLog};
use pi_session::SessionStore;
use std::fs;
use uuid::Uuid;

/// PRD mapping:
/// - test_suite.md §1.3 `session_roundtrip`: write JSONL, reload, compare normalized structure.
/// - spec.md session semantics: parent-child links and branch head updates remain intact.
#[tokio::test]
async fn session_jsonl_roundtrip_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let session_path = tmp.path().join("session.jsonl");

    let user_id = Uuid::parse_str("00000000-0000-0000-0000-000000000001").expect("uuid");
    let assistant_id = Uuid::parse_str("00000000-0000-0000-0000-000000000002").expect("uuid");

    let raw = [
        format!(
            "{{\"timestamp_ms\":1,\"schema_version\":\"1.0\",\"kind\":\"user_message\",\"text\":\"hello\",\"entry_id\":\"{user_id}\",\"parent_id\":null}}"
        ),
        format!(
            "{{\"kind\":\"assistant_message\",\"schema_version\":\"1.0\",\"entry_id\":\"{assistant_id}\",\"text\":\"hi\",\"timestamp_ms\":2,\"parent_id\":\"{user_id}\"}}"
        ),
    ]
    .join("\n")
        + "\n";
    fs::write(&session_path, &raw).expect("write session");

    let log = SessionLog::load_jsonl(&session_path).expect("load protocol log");
    assert_eq!(log.entries.len(), 2);
    assert_eq!(log.entries[0].entry_id, user_id);
    assert_eq!(log.entries[1].parent_id, Some(user_id));
    assert!(matches!(
        log.entries[1].kind,
        SessionEntryKind::AssistantMessage { .. }
    ));

    let roundtrip = log.to_jsonl_string().expect("serialize jsonl");
    assert_eq!(
        normalize_jsonl(&raw).expect("normalize input"),
        normalize_jsonl(&roundtrip).expect("normalize output")
    );

    let mut store = SessionStore::load(session_path.clone())
        .await
        .expect("load session store");
    assert_eq!(store.current_head().await, Some(assistant_id));

    assert!(store.checkout(user_id).await);
    let branch_id = store
        .append(SessionEntryKind::AssistantMessage {
            text: "branch reply".to_string(),
        })
        .await
        .expect("append branch message");

    let branched = SessionLog::load_jsonl(&session_path).expect("reload after append");
    let latest = branched.entries.last().expect("new entry");
    assert_eq!(latest.entry_id, branch_id);
    assert_eq!(latest.parent_id, Some(user_id));
    assert_eq!(branched.entries.len(), 3);
}
