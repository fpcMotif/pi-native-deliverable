use std::fs;
use std::path::PathBuf;

/// Session JSONL roundtrip should preserve semantics.
/// This test assumes `pi-protocol` exposes `SessionLog` types and `normalize()` helper.
#[test]
fn session_jsonl_roundtrip_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let session_path = tmp.path().join("session.jsonl");

    // Minimal synthetic session log. The actual schema lives in pi-protocol.
    let entries = vec![
        r#"{"schema_version":"1.0","entry_id":"00000000-0000-0000-0000-000000000001","kind":"user_message","parent_id":null,"payload":{"text":"hello"}}"#,
        r#"{"schema_version":"1.0","entry_id":"00000000-0000-0000-0000-000000000002","kind":"assistant_message","parent_id":"00000000-0000-0000-0000-000000000001","payload":{"text":"hi"}}"#,
    ];

    fs::write(&session_path, entries.join("\n") + "\n").expect("write session");

    // TODO: replace with real loader once implemented
    let raw = fs::read_to_string(&session_path).expect("read session");
    assert!(raw.contains("user_message"));
    assert!(raw.contains("assistant_message"));

    // TODO:
    // let log = pi_protocol::session::SessionLog::load_jsonl(&session_path).unwrap();
    // let roundtrip = log.to_jsonl_string().unwrap();
    // assert_eq!(pi_protocol::session::normalize_jsonl(&raw), pi_protocol::session::normalize_jsonl(&roundtrip));
}
