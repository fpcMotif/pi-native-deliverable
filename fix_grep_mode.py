import re

with open('crates/pi-search/src/lib.rs', 'r') as f:
    content = f.read()

old_grep_mode = """#[derive(Debug, Clone, PartialEq)]
pub enum GrepMode {
    PlainText,
    Regex,
}

impl Default for GrepMode {
    fn default() -> Self {
        Self::PlainText
    }
}"""

new_grep_mode = """#[derive(Debug, Clone, PartialEq, Default)]
pub enum GrepMode {
    #[default]
    PlainText,
    Regex,
}"""

content = re.sub(
    r'#\[derive\(Debug, Clone, PartialEq\)\]\npub enum GrepMode \{\n    PlainText,\n    Regex,\n\}\n\nimpl Default for GrepMode \{\n    fn default\(\) -> Self \{\n        Self::PlainText\n    \}\n\}',
    new_grep_mode,
    content,
    flags=re.DOTALL
)

with open('crates/pi-search/src/lib.rs', 'w') as f:
    f.write(content)
