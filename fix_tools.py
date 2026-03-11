import re

with open('crates/pi-tools/src/lib.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'pub struct ToolRegistry {\n    tools: std::collections::HashMap<String, Box<dyn Tool>>,\n}',
    '#[derive(Default)]\npub struct ToolRegistry {\n    tools: std::collections::HashMap<String, Box<dyn Tool>>,\n}'
)

with open('crates/pi-tools/src/lib.rs', 'w') as f:
    f.write(content)
