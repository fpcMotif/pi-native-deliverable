import re

with open('crates/pi-search/src/lib.rs', 'r') as f:
    content = f.read()

# 1. Unused structs/methods
content = content.replace("struct PersistedIndex {", "#[allow(dead_code)]\nstruct PersistedIndex {")
content = content.replace("const INDEX_FORMAT_VERSION: u32 = 1;", "#[allow(dead_code)]\nconst INDEX_FORMAT_VERSION: u32 = 1;")
content = content.replace("async fn apply_fs_event(&self, event: &notify::Event) -> SearchResult<()> {", "#[allow(dead_code)]\n    async fn apply_fs_event(&self, event: &notify::Event) -> SearchResult<()> {")
content = content.replace("async fn load_index_from_disk(&self) -> SearchResult<bool> {", "#[allow(dead_code)]\n    async fn load_index_from_disk(&self) -> SearchResult<bool> {")
content = content.replace("async fn persist_index(&self) -> SearchResult<()> {", "#[allow(dead_code)]\n    async fn persist_index(&self) -> SearchResult<()> {")
content = content.replace("async fn health_check_index(&self) -> SearchResult<()> {", "#[allow(dead_code)]\n    async fn health_check_index(&self) -> SearchResult<()> {")
content = content.replace("fn index_cache_path(&self) -> PathBuf {", "#[allow(dead_code)]\n    fn index_cache_path(&self) -> PathBuf {")

# 2. Default derivable
content = content.replace("pub enum GrepMode {\n    PlainText,", "#[derive(Default)]\npub enum GrepMode {\n    #[default]\n    PlainText,")
content = re.sub(r'impl Default for GrepMode \{\s*fn default\(\) -> Self \{\s*Self::PlainText\s*\}\s*\}', '', content)

# 3. is_none_or -> map_or
content = content.replace("scope.is_none_or(|scope| scope_is_prefix(&entry.relative_path, scope))", "scope.map_or(true, |scope| scope_is_prefix(&entry.relative_path, scope))")
content = content.replace(".is_none_or(|ext| entry.relative_path.ends_with(&format!(\".{ext}\")))", ".map_or(true, |ext| entry.relative_path.ends_with(&format!(\".{ext}\")))")
content = content.replace(".is_none_or(|prefix| scope_is_prefix(&entry.relative_path, prefix))", ".map_or(true, |prefix| scope_is_prefix(&entry.relative_path, prefix))")

# 4. Needless Ok(?)
content = content.replace("Ok(value\n        .try_into()\n        .map_err(|_| SearchError::InvalidToken(\"token overflow\".to_string()))?)", "value\n        .try_into()\n        .map_err(|_| SearchError::InvalidToken(\"token overflow\".to_string()))")


with open('crates/pi-search/src/lib.rs', 'w') as f:
    f.write(content)
