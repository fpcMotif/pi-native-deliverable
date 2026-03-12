import sys

with open("crates/pi-session/benches/session_load.rs", "r") as f:
    content = f.read()

content = content.replace(
    "let rt = Runtime::new().unwrap();",
    "let rt = Runtime::new().expect(\"Failed to create tokio runtime\");"
)
content = content.replace(
    "let mut store = SessionStore::new(path).await.unwrap();",
    "let mut store = SessionStore::new(path).await.expect(\"Failed to create SessionStore\");"
)
content = content.replace(
    """                store
                    .add_message(MessageRole::User, format!("Hello {}", i), None)
                    .await
                    .unwrap();""",
    """                store
                    .add_message(MessageRole::User, format!("Hello {}", i), None)
                    .await
                    .expect("Failed to add message");"""
)
content = content.replace(
    "let _store = SessionStore::new(path).await.unwrap();",
    "let _store = SessionStore::new(path).await.expect(\"Failed to load SessionStore\");"
)
content = content.replace(
    "std::fs::remove_file(path).unwrap();",
    "std::fs::remove_file(path).unwrap_or_else(|_| ());"
)

with open("crates/pi-session/benches/session_load.rs", "w") as f:
    f.write(content)

with open("crates/pi-session/src/bin_benchmark.rs", "r") as f:
    content = f.read()

content = content.replace(
    "let mut store = SessionStore::new(\"test_session.jsonl\").await.unwrap();",
    "let mut store = SessionStore::new(\"test_session.jsonl\").await.expect(\"Failed to create test session\");"
)
content = content.replace(
    """            .await
            .unwrap();""",
    """            .await
            .expect("Failed to add message");"""
)
content = content.replace(
    "store.compact(None).await.unwrap();",
    "store.compact(None).await.expect(\"Failed to compact\");"
)
content = content.replace(
    "std::fs::remove_file(\"test_session.jsonl\").unwrap();",
    "std::fs::remove_file(\"test_session.jsonl\").unwrap_or_else(|_| ());"
)
content = content.replace(
    "std::fs::remove_file(\"test_session.compact.jsonl\").unwrap();",
    "std::fs::remove_file(\"test_session.compact.jsonl\").unwrap_or_else(|_| ());"
)

with open("crates/pi-session/src/bin_benchmark.rs", "w") as f:
    f.write(content)
