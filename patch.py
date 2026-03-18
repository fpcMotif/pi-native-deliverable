with open("crates/pi-session/src/lib.rs", "r") as f:
    content = f.read()

test_code = r"""
    #[test]
    fn test_resolve_session_path() {
        let temp_dir = env::temp_dir();
        let workspace_root = temp_dir.join(format!("pi_workspace_{}", Uuid::new_v4()));
        fs::create_dir_all(&workspace_root).unwrap();
        // Canonicalize workspace_root so comparisons with canonicalized results match
        let workspace_root = workspace_root.canonicalize().unwrap();

        // 1. Absolute path outside workspace
        let outside_path = temp_dir.join("outside.jsonl");
        let res = SessionStore::resolve_session_path(&outside_path, &workspace_root);
        assert!(res.is_err(), "Outside path should error");

        // 2. Absolute path inside workspace
        let inside_path = workspace_root.join("inside.jsonl");
        let res = SessionStore::resolve_session_path(&inside_path, &workspace_root);
        assert!(res.is_ok(), "Inside path should be ok");
        assert_eq!(res.unwrap(), inside_path);

        // 3. Relative path
        let rel_path = PathBuf::from("rel.jsonl");
        let res = SessionStore::resolve_session_path(&rel_path, &workspace_root);
        assert!(res.is_ok(), "Relative path should be ok");
        assert_eq!(res.unwrap(), workspace_root.join("rel.jsonl"));

        // 4. Relative path escaping workspace
        let escape_path = PathBuf::from("../escape.jsonl");
        let res = SessionStore::resolve_session_path(&escape_path, &workspace_root);
        assert!(res.is_err(), "Escaping path should error");

        // 5. Symlinks (unix only, for symlink privileges safety)
        #[cfg(unix)]
        {
            let symlink_target = workspace_root.join("target_dir");
            fs::create_dir_all(&symlink_target).unwrap();
            let symlink_path = workspace_root.join("symlink_dir");
            std::os::unix::fs::symlink(&symlink_target, &symlink_path).unwrap();

            let symlink_file = symlink_path.join("symlink_file.jsonl");
            let res = SessionStore::resolve_session_path(&symlink_file, &workspace_root);
            assert!(res.is_ok(), "Symlink path should be ok");
            assert_eq!(res.unwrap(), symlink_target.join("symlink_file.jsonl"));
        }

        fs::remove_dir_all(&workspace_root).unwrap();
    }
"""

content = content.replace("mod tests {\n    use super::*;\n    use std::env;", "mod tests {\n    use super::*;\n    use std::env;\n" + test_code)

with open("crates/pi-session/src/lib.rs", "w") as f:
    f.write(content)
