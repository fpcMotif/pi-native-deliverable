with open("crates/pi-session/src/lib.rs", "r") as f:
    content = f.read()

content = content.replace(
    "let mut store = SessionStore::new(&session_path).await.unwrap();",
    "let mut store = SessionStore::new(&session_path).await.expect(\"Failed to create SessionStore\");"
)
with open("crates/pi-session/src/lib.rs", "w") as f:
    f.write(content)
