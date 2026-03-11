with open('crates/pi-search/src/lib.rs', 'r') as f:
    content = f.read()

old_str = """#[derive(Debug, Clone, Serialize, Deserialize)]
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

new_str = """#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum GrepMode {
    #[default]
    PlainText,
    Regex,
    Fuzzy,
}"""

if old_str in content:
    content = content.replace(old_str, new_str)
    with open('crates/pi-search/src/lib.rs', 'w') as f:
        f.write(content)
    print("Replaced default")
else:
    print("Could not replace default")
