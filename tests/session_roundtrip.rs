use pi_protocol::session::{normalize_jsonl, SessionEntry, SessionEntryKind, SessionLog};
use pi_session::SessionStore;
use std::fs;
use uuid::Uuid;

#[test]
fn session_jsonl_roundtrip_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let session_path = tmp.path().join("session.jsonl");

    let root_id = Uuid::new_v4();
    let child_id = Uuid::new_v4();

    let mut log = SessionLog::default();
    log.append_entry(SessionEntry {
        schema_version: "1.0".to_string(),
        entry_id: root_id,
        timestamp_ms: 1,
        kind: SessionEntryKind::UserMessage {
            text: "hello".to_string(),
        },
        parent_id: None,
        metadata: None,
    });
    log.append_entry(SessionEntry {
        schema_version: "1.0".to_string(),
        entry_id: child_id,
        timestamp_ms: 2,
        kind: SessionEntryKind::AssistantMessage {
            text: "hi".to_string(),
        },
        parent_id: Some(root_id),
        metadata: None,
    });

    let raw = log.to_jsonl_string().expect("serialize jsonl");
    fs::write(&session_path, format!("{raw}\n")).expect("write session");

    let loaded = SessionLog::load_jsonl(&session_path).expect("load jsonl");
    let roundtrip = loaded.to_jsonl_string().expect("re-serialize jsonl");

    assert_eq!(
        normalize_jsonl(&raw).expect("normalize original"),
        normalize_jsonl(&roundtrip).expect("normalize roundtrip")
    );

    let roots = loaded.roots();
    assert_eq!(roots, vec![child_id]);

    let children = loaded.children();
    assert_eq!(children.get(&root_id), Some(&vec![child_id]));
    assert!(!children.contains_key(&child_id));

    let runtime = tokio::runtime::Runtime::new().expect("runtime");
    runtime.block_on(async {
        let mut store = SessionStore::load(session_path.clone())
            .await
            .expect("store load");

        assert_eq!(store.current_head().await, Some(child_id));

        let fork_head = store.branch_from(root_id).await.expect("branch");
        assert_eq!(store.current_head().await, Some(fork_head));

        assert!(store.checkout(child_id).await);
        assert_eq!(store.current_head().await, Some(child_id));

        assert_eq!(store.continue_most_recent(), Some(fork_head));
        assert_eq!(store.current_head().await, Some(fork_head));
    });
}
