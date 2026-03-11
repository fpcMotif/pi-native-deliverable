#[cfg(test)]
mod tests {
    use crate::Policy;
    use std::path::Path;

    #[test]
    fn test_can_write_path_allowed() {
        let policy = Policy::safe_defaults(std::env::current_dir().unwrap());

        // Normal files and directories should be allowed
        assert!(policy.can_write_path(Path::new("src/main.rs")));
        assert!(policy.can_write_path(Path::new("Cargo.toml")));
        assert!(policy.can_write_path(Path::new("README.md")));
        assert!(policy.can_write_path(Path::new("tests/integration_test.rs")));
        assert!(policy.can_write_path(Path::new("docs/api/index.html")));
    }

    #[test]
    fn test_can_write_path_denied() {
        let policy = Policy::safe_defaults(std::env::current_dir().unwrap());

        // Deny list directly
        assert!(!policy.can_write_path(Path::new(".env")));
        assert!(!policy.can_write_path(Path::new(".bash_history")));

        // Hardcoded sensitive directories
        assert!(!policy.can_write_path(Path::new(".git/config")));
        assert!(!policy.can_write_path(Path::new(".ssh/known_hosts")));
        assert!(!policy.can_write_path(Path::new(".aws/credentials")));
        assert!(!policy.can_write_path(Path::new("src/.git/HEAD")));

        // Sensitive file prefixes
        assert!(!policy.can_write_path(Path::new(".env.local")));
        assert!(!policy.can_write_path(Path::new(".env.test")));
        assert!(!policy.can_write_path(Path::new("config/.env.prod")));

        // Specific SSH key file patterns
        assert!(!policy.can_write_path(Path::new("id_rsa")));
        assert!(!policy.can_write_path(Path::new("id_rsa.pub")));
        assert!(!policy.can_write_path(Path::new("id_ed25519")));
        assert!(!policy.can_write_path(Path::new("id_ed25519.pub")));
        assert!(!policy.can_write_path(Path::new("id_ecdsa")));
        assert!(!policy.can_write_path(Path::new("id_ecdsa.pub")));
        assert!(!policy.can_write_path(Path::new("id_dsa")));
        assert!(!policy.can_write_path(Path::new("id_dsa.pub")));
        assert!(!policy.can_write_path(Path::new("keys/id_rsa")));
    }

    #[test]
    fn test_can_write_path_case_insensitivity() {
        let policy = Policy::safe_defaults(std::env::current_dir().unwrap());

        // Case-insensitive matching
        assert!(!policy.can_write_path(Path::new(".ENV")));
        assert!(!policy.can_write_path(Path::new(".Git/config")));
        assert!(!policy.can_write_path(Path::new("ID_RSA")));
    }
}
