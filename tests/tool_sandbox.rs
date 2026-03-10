use pi_tools::{
    is_dangerous_command, AuditRecord, BashTool, Policy, ReadTool, Tool, ToolCall, ToolError,
    ToolRegistry, WriteTool,
};
use serde_json::json;
use std::fs;
use std::path::Path;

/// Tool sandbox: deny writing to secrets by default policy.
#[test]
fn tool_policy_denies_env_write() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let env_path = tmp.path().join(".env");

    fs::write(&env_path, "SECRET=1").expect("write");

    let policy = Policy::safe_defaults(tmp.path());
    let tool = WriteTool;
    let call = ToolCall {
        id: "write-env".to_string(),
        name: "write".to_string(),
        args: json!({
            "path": ".env",
            "content": "REDACTED=1",
        }),
    };

    let res = tool.execute(&call, &policy, tmp.path());
    assert!(matches!(res, Err(ToolError::Denied(_))));

    assert!(env_path.exists());
    let stored = fs::read_to_string(&env_path).expect("read");
    assert_eq!(stored, "SECRET=1");
}

#[test]
fn policy_presets_have_expected_capabilities() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let safe = Policy::safe_defaults(tmp.path());
    let balanced = Policy::balanced_defaults(tmp.path());
    let permissive = Policy::permissive_defaults(tmp.path());

    assert!(safe.deny_write_paths.iter().any(|v| v == ".bash_history"));
    assert!(!balanced
        .deny_write_paths
        .iter()
        .any(|v| v == ".bash_history"));
    assert!(!permissive.deny_write_paths.iter().any(|v| v == ".env"));

    assert!(safe.command_timeout_ms < balanced.command_timeout_ms);
    assert!(balanced.command_timeout_ms < permissive.command_timeout_ms);

    assert_eq!(safe.explain()["preset"], "safe");
    assert_eq!(balanced.explain()["preset"], "balanced");
    assert_eq!(permissive.explain()["preset"], "permissive");
}

#[test]
fn tool_policy_rejects_path_traversal() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let policy = Policy::safe_defaults(tmp.path());

    let escaped = policy
        .canonicalize_path("../outside.txt", tmp.path())
        .expect_err("escape should be denied");
    assert!(escaped.to_string().contains("path escapes workspace"));

    let escaped_abs = policy
        .canonicalize_path("/tmp/outside.txt", tmp.path())
        .expect_err("absolute outside path should be denied");
    assert!(escaped_abs.to_string().contains("path escapes workspace"));
}

#[test]
fn tool_policy_blocks_binary_read() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let binary = tmp.path().join("binary.bin");
    fs::write(&binary, vec![0u8, 1u8, 0u8]).expect("write");

    let policy = Policy::safe_defaults(tmp.path());
    let tool = ReadTool;
    let call = ToolCall {
        id: "read-bin".to_string(),
        name: "read".to_string(),
        args: json!({
            "path": "binary.bin",
        }),
    };

    let res = tool.execute(&call, &policy, tmp.path());
    assert!(matches!(res, Err(ToolError::Denied(msg)) if msg.contains("binary")));
}

#[test]
fn audit_emits_records_for_allowed_and_denied_actions() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let policy = Policy::safe_defaults(tmp.path()).with_session_id("sess-1");

    let mut registry = ToolRegistry::new();
    registry.register(WriteTool);

    let allowed = ToolCall {
        id: "write-ok".to_string(),
        name: "write".to_string(),
        args: json!({"path":"ok.txt","content":"ok"}),
    };
    registry
        .execute("write", &allowed, &policy, tmp.path())
        .expect("allowed write should pass");

    let denied = ToolCall {
        id: "write-denied".to_string(),
        name: "write".to_string(),
        args: json!({"path":".env","content":"nope"}),
    };
    let deny_result = registry.execute("write", &denied, &policy, tmp.path());
    assert!(matches!(deny_result, Err(ToolError::Denied(_))));

    let entries = policy.recent_audit_entries(10);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].decision, "allow");
    assert_eq!(entries[0].tool_name, "write");
    assert_eq!(entries[1].decision, "deny");
    assert_eq!(entries[1].session_id.as_deref(), Some("sess-1"));

    let raw = fs::read_to_string(tmp.path().join(".pi/tool-audit.jsonl")).expect("audit log file");
    let decoded = raw
        .lines()
        .map(|line| serde_json::from_str::<AuditRecord>(line).expect("valid audit record"))
        .collect::<Vec<_>>();
    assert_eq!(decoded.len(), 2);
}

#[test]
fn bash_dangerous_command_is_blocked() {
    let policy = Policy::safe_defaults(Path::new("/tmp"));
    let tool = BashTool;
    let call = ToolCall {
        id: "bash-danger".to_string(),
        name: "bash".to_string(),
        args: json!({
            "command": "rm -rf /tmp/unsafe",
        }),
    };

    let res = tool.execute(&call, &policy, Path::new("/tmp"));
    assert!(matches!(res, Err(ToolError::Denied(_))));
}

#[test]
fn bash_dangerous_command_detector_is_stable() {
    assert!(is_dangerous_command("rm -rf /tmp/x"));
    assert!(is_dangerous_command("mkfs /dev/sda"));
    assert!(is_dangerous_command(":(){ :|:& };:"));
    assert!(!is_dangerous_command("echo safe"));
}
