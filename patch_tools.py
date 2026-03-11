import sys
content = open("crates/pi-tools/src/lib.rs").read()

content = content.replace(
    "pub struct ToolRegistry {",
    "#[derive(Default)]\npub struct ToolRegistry {"
)

open("crates/pi-tools/src/lib.rs", "w").write(content)
