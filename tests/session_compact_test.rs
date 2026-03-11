use pi_session::SessionStore;
use pi_protocol::session::{SessionEntry, SessionEntryKind};
use std::fs;

#[tokio::test]
async fn test_compact_in_place() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let session_path = tmp.path().join("session.jsonl");
    let mut store = SessionStore::new(&session_path).await.unwrap();

    let entry = SessionEntry::new(
        SessionEntryKind::AssistantMessage {
            text: "hello".to_string(),
        },
        None,
    );
    store.append_entry(entry).await.unwrap();

    // Call compact with None. Should compact in place.
    let count = store.compact(None).await.unwrap();
    assert_eq!(count, 1);

    // The file should exist and have the compacted content
    let content = fs::read_to_string(&session_path).unwrap();
    assert!(content.contains("hello"));

    // No extra compact.jsonl file should exist
    let unexpected_path = session_path.with_extension("compact.jsonl");
    assert!(!unexpected_path.exists());
}
