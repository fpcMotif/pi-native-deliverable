import re

with open('crates/pi-search/src/lib.rs', 'r') as f:
    content = f.read()

# Fix GrepMode derive properly
old_grep = """#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrepMode {
    PlainText,
    Regex,
    Fuzzy,
}

impl Default for GrepMode {
    fn default() -> Self {
        Self::PlainText
    }
}"""

new_grep = """#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GrepMode {
    #[default]
    PlainText,
    Regex,
    Fuzzy,
}"""

content = content.replace(old_grep, new_grep)

with open('crates/pi-search/src/lib.rs', 'w') as f:
    f.write(content)
