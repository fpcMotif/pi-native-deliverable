use super::*;
use std::path::Path;

#[test]
fn test_can_write_path_allowed() {
    let policy = Policy::safe_defaults("/tmp/workspace");

    // Normal files and directories
    assert!(policy.can_write_path(Path::new("src/main.rs")));
    assert!(policy.can_write_path(Path::new("Cargo.toml")));
    assert!(policy.can_write_path(Path::new("lib/index.js")));
    assert!(policy.can_write_path(Path::new(".gitignore")));
    assert!(policy.can_write_path(Path::new("tests/integration_test.rs")));
    assert!(policy.can_write_path(Path::new("node_modules/package/index.js"))); // Unless explicitly denied
}

#[test]
fn test_can_write_path_hardcoded_sensitive_directories() {
    let policy = Policy::safe_defaults("/tmp/workspace");

    // Exact directory match
    assert!(!policy.can_write_path(Path::new(".git")));
    assert!(!policy.can_write_path(Path::new(".ssh")));
    assert!(!policy.can_write_path(Path::new(".aws")));

    // Sub-paths
    assert!(!policy.can_write_path(Path::new(".git/config")));
    assert!(!policy.can_write_path(Path::new(".ssh/id_rsa")));
    assert!(!policy.can_write_path(Path::new(".aws/credentials")));

    // Nested sensitive directories
    assert!(!policy.can_write_path(Path::new("some/path/.git/config")));
    assert!(!policy.can_write_path(Path::new("user/home/.ssh/id_rsa")));

    // Case insensitivity
    assert!(!policy.can_write_path(Path::new(".GIT/config")));
    assert!(!policy.can_write_path(Path::new(".SSH")));
    assert!(!policy.can_write_path(Path::new(".Aws/config")));
}

#[test]
fn test_can_write_path_sensitive_prefixes() {
    let policy = Policy::safe_defaults("/tmp/workspace");

    // .env files
    assert!(!policy.can_write_path(Path::new(".env")));
    assert!(!policy.can_write_path(Path::new(".env.local")));
    assert!(!policy.can_write_path(Path::new(".env.production")));

    // Nested .env files
    assert!(!policy.can_write_path(Path::new("backend/.env")));
    assert!(!policy.can_write_path(Path::new("config/.env.test")));

    // Case insensitivity
    assert!(!policy.can_write_path(Path::new(".ENV")));
    assert!(!policy.can_write_path(Path::new(".Env.dev")));
}

#[test]
fn test_can_write_path_ssh_keys() {
    let policy = Policy::safe_defaults("/tmp/workspace");

    let ssh_keys = [
        "id_rsa",
        "id_rsa.pub",
        "id_ed25519",
        "id_ed25519.pub",
        "id_ecdsa",
        "id_ecdsa.pub",
        "id_dsa",
        "id_dsa.pub",
    ];

    for key in ssh_keys {
        assert!(
            !policy.can_write_path(Path::new(key)),
            "Failed for key: {}",
            key
        );
        assert!(
            !policy.can_write_path(Path::new(&format!("keys/{}", key))),
            "Failed for nested key: {}",
            key
        );
        assert!(
            !policy.can_write_path(Path::new(&key.to_uppercase())),
            "Failed for uppercase key: {}",
            key
        );
    }
}

#[test]
fn test_can_write_path_deny_list() {
    let mut policy = Policy::safe_defaults("/tmp/workspace");
    policy.deny_write_paths = vec![
        "node_modules".to_string(),
        "target".to_string(),
        "build.js".to_string(),
    ];

    // Exact matches
    assert!(!policy.can_write_path(Path::new("node_modules")));
    assert!(!policy.can_write_path(Path::new("target")));
    assert!(!policy.can_write_path(Path::new("build.js")));

    // Nested paths
    assert!(!policy.can_write_path(Path::new("node_modules/express/index.js")));
    assert!(!policy.can_write_path(Path::new("project/target/release/app")));
    assert!(!policy.can_write_path(Path::new("scripts/build.js")));

    // Case insensitivity
    assert!(!policy.can_write_path(Path::new("NODE_MODULES")));
    assert!(!policy.can_write_path(Path::new("Target/debug")));
    assert!(!policy.can_write_path(Path::new("BUILD.JS")));

    // Still allowed paths
    assert!(policy.can_write_path(Path::new("src/main.rs")));
    assert!(policy.can_write_path(Path::new("package.json")));
}
