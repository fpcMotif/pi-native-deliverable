import re

with open('crates/pi-tools/src/lib.rs', 'r') as f:
    content = f.read()

replacement = """impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {"""

content = content.replace("impl ToolRegistry {", replacement)

with open('crates/pi-tools/src/lib.rs', 'w') as f:
    f.write(content)
