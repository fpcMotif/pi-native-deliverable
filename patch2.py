with open("crates/pi-tools/src/bash_test.rs", "r") as f:
    content = f.read()

content = content.replace(
    "let policy = Policy::safe_defaults(std::env::current_dir().unwrap());",
    "let policy = Policy::safe_defaults(std::env::current_dir().expect(\"current_dir\"));"
)
content = content.replace(
    ".execute(&call, &policy, std::path::Path::new(\".\"))\n            .unwrap();",
    ".execute(&call, &policy, std::path::Path::new(\".\"))\n            .expect(\"execute\");"
)
content = content.replace(
    "let mut policy = Policy::safe_defaults(std::env::current_dir().unwrap());",
    "let mut policy = Policy::safe_defaults(std::env::current_dir().expect(\"current_dir\"));"
)
with open("crates/pi-tools/src/bash_test.rs", "w") as f:
    f.write(content)
