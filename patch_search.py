import sys
content = open("crates/pi-search/src/lib.rs").read()

content = content.replace(
    'scope.is_none_or(|scope| scope_is_prefix(&entry.relative_path, scope))',
    'scope.map_or(true, |scope| scope_is_prefix(&entry.relative_path, scope))'
)

content = content.replace(
    '.is_none_or(|ext| entry.relative_path.ends_with(&format!(".{ext}")));',
    '.map_or(true, |ext| entry.relative_path.ends_with(&format!(".{ext}")));'
)

content = content.replace(
    '.is_none_or(|prefix| scope_is_prefix(&entry.relative_path, prefix));',
    '.map_or(true, |prefix| scope_is_prefix(&entry.relative_path, prefix));'
)

content = content.replace(
    """    Ok(value
        .try_into()
        .map_err(|_| SearchError::InvalidToken("token overflow".to_string()))?)""",
    """    value
        .try_into()
        .map_err(|_| SearchError::InvalidToken("token overflow".to_string()))"""
)

# Replace the GrepMode Default impl
grep_mode_def = """impl Default for GrepMode {
    fn default() -> Self {
        Self::PlainText
    }
}"""
content = content.replace(grep_mode_def, "")

grep_mode_enum = """pub enum GrepMode {
    PlainText,
    Regex,"""
grep_mode_enum_new = """#[derive(Default)]
pub enum GrepMode {
    #[default]
    PlainText,
    Regex,"""
content = content.replace(grep_mode_enum, grep_mode_enum_new)

# Suppress dead code warnings for PersistedIndex and methods
content = content.replace("struct PersistedIndex {", "#[allow(dead_code)]\nstruct PersistedIndex {")
content = content.replace("const INDEX_FORMAT_VERSION: u32 = 1;", "#[allow(dead_code)]\nconst INDEX_FORMAT_VERSION: u32 = 1;")
content = content.replace("async fn apply_fs_event", "#[allow(dead_code)]\n    async fn apply_fs_event")
content = content.replace("async fn load_index_from_disk", "#[allow(dead_code)]\n    async fn load_index_from_disk")
content = content.replace("async fn persist_index", "#[allow(dead_code)]\n    async fn persist_index")
content = content.replace("async fn health_check_index", "#[allow(dead_code)]\n    async fn health_check_index")
content = content.replace("fn index_cache_path", "#[allow(dead_code)]\n    fn index_cache_path")

open("crates/pi-search/src/lib.rs", "w").write(content)
