import re

with open("crates/pi-search/src/lib.rs", "r") as f:
    content = f.read()

# Fix dead code
content = re.sub(
    r'struct PersistedIndex \{',
    r'#[allow(dead_code)]\nstruct PersistedIndex {',
    content
)
content = re.sub(
    r'const INDEX_FORMAT_VERSION: u32 = 1;',
    r'#[allow(dead_code)]\nconst INDEX_FORMAT_VERSION: u32 = 1;',
    content
)
content = re.sub(
    r'    async fn apply_fs_event',
    r'    #[allow(dead_code)]\n    async fn apply_fs_event',
    content
)
content = re.sub(
    r'    async fn load_index_from_disk',
    r'    #[allow(dead_code)]\n    async fn load_index_from_disk',
    content
)
content = re.sub(
    r'    async fn persist_index',
    r'    #[allow(dead_code)]\n    async fn persist_index',
    content
)
content = re.sub(
    r'    async fn health_check_index',
    r'    #[allow(dead_code)]\n    async fn health_check_index',
    content
)
content = re.sub(
    r'    fn index_cache_path',
    r'    #[allow(dead_code)]\n    fn index_cache_path',
    content
)


# Fix default impl
content = re.sub(
    r'pub enum GrepMode {\n    PlainText,\n    Regex,\n}\n\nimpl Default for GrepMode {\n    fn default\(\) -> Self {\n        Self::PlainText\n    }\n}',
    r'#[derive(Default)]\npub enum GrepMode {\n    #[default]\n    PlainText,\n    Regex,\n}',
    content
)

# Fix is_none_or -> map_or
content = re.sub(
    r'\.is_none_or\(\|scope\|',
    r'.map_or(true, |scope|',
    content
)
content = re.sub(
    r'\.is_none_or\(\|ext\|',
    r'.map_or(true, |ext|',
    content
)
content = re.sub(
    r'\.is_none_or\(\|prefix\|',
    r'.map_or(true, |prefix|',
    content
)

# Fix needless question mark
content = re.sub(
    r'    Ok\(value\n        \.try_into\(\)\n        \.map_err\(\|\_\| SearchError::InvalidToken\("token overflow"\.to_string\(\)\)\)\?\)',
    r'    value\n        .try_into()\n        .map_err(|_| SearchError::InvalidToken("token overflow".to_string()))',
    content
)

# Fix items after test module
content = re.sub(
    r'mod tests \{',
    r'#[allow(clippy::items_after_test_module)]\nmod tests {',
    content
)


with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(content)
