use pi_tools::ToolError;
use pi_tools::{is_dangerous_command, BashTool, Policy, ReadTool, Tool, ToolCall, WriteTool};
use serde_json::json;
use std::fs;
use std::path::Path;

/// Tool sandbox: deny writing to secrets by default policy.
#[test]
fn tool_policy_denies_env_write() {
    let tmp = tempfile::tempdir().expect("tempdir");

    let policy = pi_tools::Policy::safe_defaults(tmp.path());
    let tool = WriteTool;
    let call = ToolCall {
        id: "write-env".to_string(),
        name: "write".to_string(),
        args: serde_json::json!({
            "path": ".env",
            "content": "SECRET=1",
        }),
    };

    let res = tool.execute(&call, &policy, tmp.path()).unwrap_err();
    assert!(res.to_string().contains("denied"));
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
            "max_bytes": 100,
        }),
    };

    let res = tool.execute(&call, &policy, tmp.path());
    println!("read tool result: {:?}", res);
    assert!(matches!(res, Err(ToolError::Denied(msg)) if msg.contains("binary")));
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

#[test]
fn policy_presets_are_named_and_distinct() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let safe = Policy::safe(tmp.path());
    let balanced = Policy::balanced(tmp.path());
    let permissive = Policy::permissive(tmp.path());

    assert_eq!(safe.preset, pi_tools::PolicyPreset::Safe);
    assert_eq!(balanced.preset, pi_tools::PolicyPreset::Balanced);
    assert_eq!(permissive.preset, pi_tools::PolicyPreset::Permissive);
    assert!(!safe.deny_write_paths.is_empty());
    assert!(permissive.deny_write_paths.is_empty());
}
