#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use crate::Policy;

    #[test]
    fn test_can_write_path() {
        let mut policy = Policy::safe(PathBuf::from("/tmp"));
        policy.deny_write_paths = vec!["target".to_string(), "node_modules".to_string()];

        // Allowed paths
        assert!(policy.can_write_path(&PathBuf::from("src/main.rs")));
        assert!(policy.can_write_path(&PathBuf::from("Cargo.toml")));
        assert!(policy.can_write_path(&PathBuf::from("foo/bar/baz.txt")));
        assert!(policy.can_write_path(&PathBuf::from("tests/integration_test.rs")));

        // Deny list paths
        assert!(!policy.can_write_path(&PathBuf::from("target/debug/foo")));
        assert!(!policy.can_write_path(&PathBuf::from("foo/node_modules/bar")));
        assert!(!policy.can_write_path(&PathBuf::from("NODE_MODULES/index.js")));
        assert!(!policy.can_write_path(&PathBuf::from("Target/foo.txt")));

        // Hardcoded sensitive directories
        assert!(!policy.can_write_path(&PathBuf::from(".git/config")));
        assert!(!policy.can_write_path(&PathBuf::from("foo/.ssh/id_rsa")));
        assert!(!policy.can_write_path(&PathBuf::from(".aws/credentials")));
        assert!(!policy.can_write_path(&PathBuf::from(".GIT/index")));

        // Env files
        assert!(!policy.can_write_path(&PathBuf::from(".env")));
        assert!(!policy.can_write_path(&PathBuf::from(".env.local")));
        assert!(!policy.can_write_path(&PathBuf::from("foo/.env.test")));

        // SSH keys
        assert!(!policy.can_write_path(&PathBuf::from("id_rsa")));
        assert!(!policy.can_write_path(&PathBuf::from("id_rsa.pub")));
        assert!(!policy.can_write_path(&PathBuf::from("foo/id_ed25519")));
        assert!(!policy.can_write_path(&PathBuf::from("bar/id_ecdsa.pub")));

        // Not SSH keys but close
        assert!(policy.can_write_path(&PathBuf::from("id_rsa_foo")));
        assert!(policy.can_write_path(&PathBuf::from("my_id_rsa")));
    }
}
