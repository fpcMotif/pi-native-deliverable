use pi_protocol::session::{SessionEntry, SessionEntryKind};
use pi_session::SessionStore;
use std::fs;
use std::path::{Path, PathBuf};

async fn store_with_message(session_path: &Path, text: &str) -> SessionStore {
    let mut store = SessionStore::new(session_path).await.unwrap();
    let entry = SessionEntry::new(
        SessionEntryKind::AssistantMessage {
            text: text.to_string(),
        },
        None,
    );
    store.append_entry(entry).await.unwrap();
    store
}

fn read(path: PathBuf) -> String {
    fs::read_to_string(path).unwrap()
}

#[tokio::test]
async fn compact_without_target_rewrites_session_in_place() {
    let tmp = tempfile::tempdir().unwrap();
    let session_path = tmp.path().join("session.jsonl");
    let mut store = store_with_message(&session_path, "hello").await;

    let count = store.compact(None).await.unwrap();

    assert_eq!(count, 1);
    assert!(read(session_path.clone()).contains("hello"));
    assert!(!session_path.with_extension("compact.jsonl").exists());
}

#[tokio::test]
async fn compact_with_explicit_target_keeps_session_file_in_sync() {
    let tmp = tempfile::tempdir().unwrap();
    let session_path = tmp.path().join("session.jsonl");
    let compact_path = tmp.path().join("archive.jsonl");
    let mut store = store_with_message(&session_path, "hello").await;

    let count = store.compact(Some(&compact_path)).await.unwrap();

    assert_eq!(count, 1);
    assert_eq!(read(session_path), read(compact_path));
}
