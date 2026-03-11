use pi_tools::{Policy, PolicyPreset, DEFAULT_WRITE_LIMIT_BYTES};
use std::path::PathBuf;

#[test]
fn test_safe_defaults() {
    let workspace_root = PathBuf::from("/test/workspace");
    let policy = Policy::safe_defaults(workspace_root.clone());

    assert_eq!(policy.preset, PolicyPreset::Safe);
    assert_eq!(policy.workspace_root, workspace_root);
    assert_eq!(policy.max_stdout_bytes, 32 * 1024);
    assert_eq!(policy.max_stderr_bytes, 8 * 1024);
    assert_eq!(policy.command_timeout_ms, 5_000);
    assert_eq!(
        policy.deny_write_paths,
        vec![
            ".env".to_string(),
            ".env.local".to_string(),
            ".bash_history".to_string(),
            "id_rsa".to_string(),
            "id_rsa.pub".to_string(),
        ]
    );
    assert_eq!(policy.max_file_size, DEFAULT_WRITE_LIMIT_BYTES);
}
