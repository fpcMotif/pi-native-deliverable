use std::fs;

use pi_protocol::session::{normalize_jsonl, SessionEntryKind, SessionLog};

#[test]
fn session_jsonl_roundtrip_semantics() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let session_path = tmp.path().join("session.jsonl");

    let entries = vec![
        r#"{"schema_version":"1.0","entry_id":"00000000-0000-0000-0000-000000000001","timestamp_ms":1,"kind":{"kind":"user_message","text":"hello"},"parent_id":null,"metadata":null}"#,
        r#"{"schema_version":"1.0","entry_id":"00000000-0000-0000-0000-000000000002","timestamp_ms":2,"kind":{"kind":"session_metadata","payload":{"type":"usage","provider":"mock","model":"gpt-test","timing":{"turn_elapsed_ms":9},"token":{"turn":{"input":11,"output":7,"cached":1},"session":{"input":20,"output":14,"cached":3}},"cost":{"turn_usd":0.01,"session_usd":0.03}}},"parent_id":"00000000-0000-0000-0000-000000000001","metadata":null}"#,
        r#"{"schema_version":"1.0","entry_id":"00000000-0000-0000-0000-000000000003","timestamp_ms":3,"kind":{"kind":"assistant_message","text":"hi"},"parent_id":"00000000-0000-0000-0000-000000000002","metadata":null}"#,
    ];

    fs::write(&session_path, entries.join("\n") + "\n").expect("write session");

    let raw = fs::read_to_string(&session_path).expect("read session");
    let log = SessionLog::load_jsonl(&session_path).expect("load session jsonl");

    match &log.entries[1].kind {
        SessionEntryKind::SessionMetadata { payload } => {
            assert_eq!(payload["provider"], "mock");
            assert_eq!(payload["model"], "gpt-test");
            assert_eq!(payload["timing"]["turn_elapsed_ms"], 9);
            assert_eq!(payload["token"]["session"]["input"], 20);
            assert_eq!(payload["cost"]["session_usd"], 0.03);
        }
        other => panic!("unexpected second entry kind: {other:?}"),
    }

    let roundtrip = log.to_jsonl_string().expect("roundtrip serialize");
    assert_eq!(
        normalize_jsonl(&raw).expect("normalize raw"),
        normalize_jsonl(&roundtrip).expect("normalize roundtrip")
    );
}
