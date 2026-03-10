use pi_protocol::{normalize_jsonl, SessionEntryKind, SessionLog};
use pi_session::SessionStore;
use std::path::Path;

#[tokio::test]
async fn session_jsonl_roundtrip_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    copy_fixture_dir(Path::new("fixtures/repo_small"), tmp.path());

    let session_path = SessionStore::resolve_session_path(".pi/session.jsonl", tmp.path())
        .expect("resolve session path");
    let mut store = SessionStore::new(session_path.clone())
        .await
        .expect("open session store");

    let user_id = store
        .append(SessionEntryKind::UserMessage {
            text: "hello".to_string(),
        })
        .await
        .expect("append user entry");

    let assistant_id = store
        .append_entry(pi_protocol::SessionEntry::new(
            SessionEntryKind::AssistantMessage {
                text: "hi".to_string(),
            },
            Some(user_id),
        ))
        .await
        .expect("append assistant entry");

    assert!(store
        .log
        .entries
        .iter()
        .any(|entry| entry.entry_id == assistant_id));

    let raw = tokio::fs::read_to_string(&session_path)
        .await
        .expect("read persisted session");
    let loaded = SessionLog::load_jsonl(&session_path).expect("load jsonl via pi-protocol");
    let roundtrip = loaded.to_jsonl_string().expect("serialize roundtrip");

    assert_eq!(loaded.entries.len(), 2);
    assert_eq!(
        normalize_jsonl(&raw).expect("normalize raw"),
        normalize_jsonl(&roundtrip).expect("normalize roundtrip")
    );
    assert!(matches!(
        loaded.entries[0].kind,
        SessionEntryKind::UserMessage { .. }
    ));
    assert!(matches!(
        loaded.entries[1].kind,
        SessionEntryKind::AssistantMessage { .. }
    ));
}

fn copy_fixture_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create target fixture dir");
    for entry in std::fs::read_dir(src).expect("read fixture dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let target = dst.join(entry.file_name());
        if path.is_dir() {
            copy_fixture_dir(&path, &target);
        } else {
            std::fs::copy(&path, &target).unwrap_or_else(|err| {
                panic!(
                    "copy {} -> {} failed: {err}",
                    path.display(),
                    target.display()
                )
            });
        }
    }
}
