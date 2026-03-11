#[cfg(test)]
mod tests {
    use crate::Policy;
    use std::path::Path;

    #[test]
    fn test_can_write_path_allowed() {
        let policy = Policy::safe_defaults("/tmp/workspace");

        assert!(policy.can_write_path(Path::new("src/main.rs")));
        assert!(policy.can_write_path(Path::new("Cargo.toml")));
        assert!(policy.can_write_path(Path::new("README.md")));
        assert!(policy.can_write_path(Path::new("tests/integration_test.rs")));
    }

    #[test]
    fn test_can_write_path_denied_hardcoded() {
        let policy = Policy::safe_defaults("/tmp/workspace");

        // Hardcoded sensitive directories
        assert!(!policy.can_write_path(Path::new(".git/config")));
        assert!(!policy.can_write_path(Path::new(".ssh/id_rsa")));
        assert!(!policy.can_write_path(Path::new(".aws/credentials")));

        // Sensitive file prefixes
        assert!(!policy.can_write_path(Path::new(".env")));
        assert!(!policy.can_write_path(Path::new(".env.local")));
        assert!(!policy.can_write_path(Path::new(".env.production")));

        // Specific SSH key file patterns
        assert!(!policy.can_write_path(Path::new("id_rsa")));
        assert!(!policy.can_write_path(Path::new("id_rsa.pub")));
        assert!(!policy.can_write_path(Path::new("id_ed25519")));
        assert!(!policy.can_write_path(Path::new("id_ecdsa")));
        assert!(!policy.can_write_path(Path::new("id_dsa")));
    }

    #[test]
    fn test_can_write_path_deny_list() {
        let mut policy = Policy::safe_defaults("/tmp/workspace");
        policy.deny_write_paths = vec!["Cargo.toml".to_string(), "secrets.json".to_string()];

        assert!(!policy.can_write_path(Path::new("Cargo.toml")));
        assert!(!policy.can_write_path(Path::new("secrets.json")));
        assert!(!policy.can_write_path(Path::new("dir/secrets.json")));

        // Case-insensitive match check
        assert!(!policy.can_write_path(Path::new("SECRETS.JSON")));

        // Still allowed
        assert!(policy.can_write_path(Path::new("src/main.rs")));
    }
}
