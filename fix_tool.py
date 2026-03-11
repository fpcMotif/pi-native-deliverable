with open("crates/pi-tools/src/lib.rs", "r") as f:
    content = f.read()

content = content.replace("    #[allow(dead_code)]\n    pub fn new() -> Self {", "    #[allow(clippy::new_without_default)]\n    pub fn new() -> Self {")

with open("crates/pi-tools/src/lib.rs", "w") as f:
    f.write(content)
