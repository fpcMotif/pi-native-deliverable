#[cfg(test)]
mod tests {
    use crate::Policy;
    use std::path::Path;

    #[test]
    fn test_can_write_path() {
        let policy = Policy::safe_defaults(std::env::current_dir().unwrap());

        // Allowed paths
        assert!(policy.can_write_path(Path::new("src/main.rs")));
        assert!(policy.can_write_path(Path::new("tests/foo.rs")));
        assert!(policy.can_write_path(Path::new("cargo.toml")));

        // Denied paths from deny_write_paths
        assert!(!policy.can_write_path(Path::new(".env")));
        assert!(!policy.can_write_path(Path::new(".env.local")));
        assert!(!policy.can_write_path(Path::new(".bash_history")));
        assert!(!policy.can_write_path(Path::new("id_rsa")));
        assert!(!policy.can_write_path(Path::new("id_rsa.pub")));

        // Hardcoded sensitive directories
        assert!(!policy.can_write_path(Path::new(".git/config")));
        assert!(!policy.can_write_path(Path::new(".ssh/authorized_keys")));
        assert!(!policy.can_write_path(Path::new(".aws/credentials")));
        assert!(!policy.can_write_path(Path::new("src/.git/config"))); // nested

        // Sensitive file prefixes
        assert!(!policy.can_write_path(Path::new(".env.production")));
        assert!(!policy.can_write_path(Path::new(".env.test")));

        // Specific SSH key patterns
        assert!(!policy.can_write_path(Path::new("id_ed25519")));
        assert!(!policy.can_write_path(Path::new("id_ed25519.pub")));
        assert!(!policy.can_write_path(Path::new("id_ecdsa")));
        assert!(!policy.can_write_path(Path::new("id_ecdsa.pub")));
        assert!(!policy.can_write_path(Path::new("id_dsa")));
        assert!(!policy.can_write_path(Path::new("id_dsa.pub")));
    }
}
