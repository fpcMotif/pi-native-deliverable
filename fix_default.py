import re

with open("crates/pi-search/src/lib.rs", "r") as f:
    content = f.read()

content = re.sub(
    r'pub enum GrepMode \{\n    PlainText,\n    Regex,\n    Fuzzy,\n\}',
    r'#[derive(Default)]\npub enum GrepMode {\n    #[default]\n    PlainText,\n    Regex,\n    Fuzzy,\n}',
    content
)

with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(content)
