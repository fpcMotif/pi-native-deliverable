import sys

file_path = "crates/pi-session/src/lib.rs"
with open(file_path, "r") as f:
    content = f.read()

new_test = """
    #[tokio::test]
    async fn test_checkout_existing_entry() {
        let temp_dir = env::temp_dir();
        let session_path = temp_dir.join(format!("{}.jsonl", Uuid::new_v4()));

        let mut store = SessionStore::new(&session_path).await.unwrap();

        // Append a new entry
        let kind = SessionEntryKind::UserMessage {
            text: "Hello".to_string(),
        };
        let entry_id = store.append(kind).await.unwrap();

        assert_eq!(store.get_branch_head(), Some(entry_id));

        // Append another entry
        let kind2 = SessionEntryKind::UserMessage {
            text: "World".to_string(),
        };
        let entry_id2 = store.append(kind2).await.unwrap();

        assert_eq!(store.get_branch_head(), Some(entry_id2));

        // Checkout the first entry
        let result = store.checkout(entry_id).await;

        // It should succeed and head_id should be updated
        assert!(result);
        assert_eq!(store.get_branch_head(), Some(entry_id));

        let _ = fs::remove_file(session_path);
    }
"""

if "test_checkout_existing_entry" not in content:
    content = content.replace("mod tests {\n    use super::*;\n    use std::env;", "mod tests {\n    use super::*;\n    use std::env;\n" + new_test)

with open(file_path, "w") as f:
    f.write(content)
print("Test added.")
