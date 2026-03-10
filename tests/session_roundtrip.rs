use pi_protocol::session::{normalize_jsonl, SessionEntry, SessionEntryKind, SessionLog};

#[test]
fn session_jsonl_roundtrip_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let session_path = tmp.path().join("session.jsonl");

    let user = SessionEntry::new(
        SessionEntryKind::UserMessage {
            text: "hello".to_string(),
        },
        None,
    );
    let assistant = SessionEntry::new(
        SessionEntryKind::AssistantMessage {
            text: "hi".to_string(),
        },
        Some(user.entry_id),
    );
    let log = SessionLog {
        entries: vec![user, assistant],
    };

    let raw = format!("{}\n", log.to_jsonl_string().expect("to jsonl"));
    std::fs::write(&session_path, &raw).expect("write session");

    let loaded = SessionLog::load_jsonl(&session_path).expect("load jsonl");
    let roundtrip = loaded.to_jsonl_string().expect("roundtrip jsonl");

    assert_eq!(
        normalize_jsonl(&raw).expect("normalize raw"),
        normalize_jsonl(&roundtrip).expect("normalize roundtrip")
    );
}
