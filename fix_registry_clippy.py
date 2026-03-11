import re

with open("crates/pi-tools/src/lib.rs", "r") as f:
    code = f.read()

replacement = r"""#[derive(Default)]
pub struct ToolRegistry {
    tools: std::collections::HashMap<String, Box<dyn Tool>>,
}"""

code = code.replace(
    "pub struct ToolRegistry {\n    tools: std::collections::HashMap<String, Box<dyn Tool>>,\n}",
    replacement
)

with open("crates/pi-tools/src/lib.rs", "w") as f:
    f.write(code)
