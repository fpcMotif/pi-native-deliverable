import re

with open('crates/pi-search/src/lib.rs', 'r') as f:
    content = f.read()

# Fix dead code
content = re.sub(
    r'(struct PersistedIndex)',
    r'#[allow(dead_code)]\n\1',
    content
)

content = re.sub(
    r'(const INDEX_FORMAT_VERSION)',
    r'#[allow(dead_code)]\n\1',
    content
)

content = re.sub(
    r'(async fn apply_fs_event)',
    r'#[allow(dead_code)]\n    \1',
    content
)

content = re.sub(
    r'(async fn load_index_from_disk)',
    r'#[allow(dead_code)]\n    \1',
    content
)

content = re.sub(
    r'(async fn persist_index)',
    r'#[allow(dead_code)]\n    \1',
    content
)

content = re.sub(
    r'(async fn health_check_index)',
    r'#[allow(dead_code)]\n    \1',
    content
)

content = re.sub(
    r'(fn index_cache_path)',
    r'#[allow(dead_code)]\n    \1',
    content
)

# Fix derivable_impls for GrepMode
# We'll use a string replace because it's a multiline block
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

content = content.replace(old_grep_mode, new_grep_mode)


# Fix MSRV is_none_or
content = content.replace(
    'scope.is_none_or(|scope| scope_is_prefix(&entry.relative_path, scope))',
    'scope.map_or(true, |scope| scope_is_prefix(&entry.relative_path, scope))'
)
content = content.replace(
    '.is_none_or(|ext| entry.relative_path.ends_with(&format!(".{ext}")))',
    '.map_or(true, |ext| entry.relative_path.ends_with(&format!(".{ext}")))'
)
content = content.replace(
    '.is_none_or(|prefix| scope_is_prefix(&entry.relative_path, prefix))',
    '.map_or(true, |prefix| scope_is_prefix(&entry.relative_path, prefix))'
)

# Fix needless_question_mark
old_ok = """    Ok(value
        .try_into()
        .map_err(|_| SearchError::InvalidToken("token overflow".to_string()))?)"""

new_ok = """    value
        .try_into()
        .map_err(|_| SearchError::InvalidToken("token overflow".to_string()))"""

content = content.replace(old_ok, new_ok)


with open('crates/pi-search/src/lib.rs', 'w') as f:
    f.write(content)
