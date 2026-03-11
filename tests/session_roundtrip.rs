use std::fs;

/// Session JSONL roundtrip should preserve semantics.
/// This test assumes `pi-protocol` exposes `SessionLog` types and `normalize()` helper.
#[test]
fn session_jsonl_roundtrip_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let session_path = tmp.path().join("session.jsonl");

    // Minimal synthetic session log. The actual schema lives in pi-protocol.
    let entries = vec![
        r#"{"schema_version":"1.0","entry_id":"00000000-0000-0000-0000-000000000001","timestamp_ms":0,"kind":{"kind":"user_message","text":"hello"},"parent_id":null,"metadata":null}"#,
        r#"{"schema_version":"1.0","entry_id":"00000000-0000-0000-0000-000000000002","timestamp_ms":0,"kind":{"kind":"assistant_message","text":"hi"},"parent_id":"00000000-0000-0000-0000-000000000001","metadata":null}"#,
    ];

    fs::write(&session_path, entries.join("\n") + "\n").expect("write session");

    let raw = fs::read_to_string(&session_path).expect("read session");
    assert!(raw.contains("user_message"));
    assert!(raw.contains("assistant_message"));

    let log = pi_protocol::session::SessionLog::load_jsonl(&session_path).unwrap();
    let roundtrip = log.to_jsonl_string().unwrap();
    assert_eq!(pi_protocol::session::normalize_jsonl(&raw).unwrap(), pi_protocol::session::normalize_jsonl(&roundtrip).unwrap());
}
