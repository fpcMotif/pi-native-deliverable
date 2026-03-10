use pi_protocol::session::normalize_jsonl;
use pi_session::SessionStore;
use std::fs;

/// Session JSONL roundtrip should preserve semantics through `pi-session` APIs.
#[tokio::test]
async fn session_jsonl_roundtrip_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let session_path = tmp.path().join("session.jsonl");

    let raw = [
        r#"{"schema_version":"1.0","entry_id":"00000000-0000-0000-0000-000000000001","timestamp_ms":1,"kind":{"kind":"user_message","text":"hello"},"parent_id":null}"#,
        r#"{"schema_version":"1.0","entry_id":"00000000-0000-0000-0000-000000000002","timestamp_ms":2,"kind":{"kind":"assistant_message","text":"hi"},"parent_id":"00000000-0000-0000-0000-000000000001"}"#,
    ]
    .join("\n");
    fs::write(&session_path, format!("{raw}\n")).expect("write session");

    let store = SessionStore::load(&session_path)
        .await
        .expect("load session");
    assert_eq!(store.log.entries.len(), 2);
    assert_eq!(store.get_branch_head(), Some(store.log.entries[1].entry_id));

    let (_roots, children) = store.load_tree();
    assert_eq!(
        children.get(&store.log.entries[0].entry_id),
        Some(&vec![store.log.entries[1].entry_id])
    );

    let roundtrip = store.to_jsonl_string().expect("to jsonl");
    assert_eq!(
        normalize_jsonl(&raw).expect("normalize input"),
        normalize_jsonl(&roundtrip).expect("normalize roundtrip")
    );
}
