import re

# 1. pi-search/src/lib.rs
with open("crates/pi-search/src/lib.rs", "r") as f:
    content = f.read()

# Fix GrepMode Default
content = re.sub(
    r"impl Default for GrepMode \{\n\s*fn default\(\) -> Self \{\n\s*Self::PlainText\n\s*\}\n\}",
    "",
    content
)
content = content.replace(
    "pub enum GrepMode {",
    "#[derive(Default)]\npub enum GrepMode {"
)
content = content.replace(
    "    PlainText,",
    "    #[default]\n    PlainText,"
)

# Fix is_none_or
content = content.replace(".is_none_or(|scope|", ".map_or(true, |scope|")
content = content.replace(".is_none_or(|ext|", ".map_or(true, |ext|")
content = content.replace(".is_none_or(|prefix|", ".map_or(true, |prefix|")

# Fix needless_question_mark
content = content.replace(
    "Ok(value\n        .try_into()\n        .map_err(|_| SearchError::InvalidToken(\"token overflow\".to_string()))?)",
    "value\n        .try_into()\n        .map_err(|_| SearchError::InvalidToken(\"token overflow\".to_string()))"
)

# Fix dead code in pi-search
content = content.replace("struct PersistedIndex {", "#[allow(dead_code)]\nstruct PersistedIndex {")
content = content.replace("const INDEX_FORMAT_VERSION: u32 = 1;", "#[allow(dead_code)]\nconst INDEX_FORMAT_VERSION: u32 = 1;")
content = content.replace("async fn apply_fs_event(", "#[allow(dead_code)]\n    async fn apply_fs_event(")
content = content.replace("async fn load_index_from_disk(", "#[allow(dead_code)]\n    async fn load_index_from_disk(")
content = content.replace("async fn persist_index(", "#[allow(dead_code)]\n    async fn persist_index(")
content = content.replace("async fn health_check_index(", "#[allow(dead_code)]\n    async fn health_check_index(")
content = content.replace("fn index_cache_path(", "#[allow(dead_code)]\n    fn index_cache_path(")

with open("crates/pi-search/src/lib.rs", "w") as f:
    f.write(content)

# 2. pi-core/src/agent.rs
with open("crates/pi-core/src/agent.rs", "r") as f:
    content = f.read()

content = content.replace("enum Command {", "#[allow(dead_code)]\nenum Command {")

with open("crates/pi-core/src/agent.rs", "w") as f:
    f.write(content)

# 3. src/main.rs
with open("src/main.rs", "r") as f:
    content = f.read()

content = content.replace("use tokio::io::{self as tokio_io, AsyncBufReadExt, AsyncWriteExt};", "use tokio::io::{self as tokio_io, AsyncBufReadExt};")
content = content.replace("async fn apply_startup_session_controls(", "#[allow(dead_code)]\nasync fn apply_startup_session_controls(")
content = content.replace("async fn print_events_to_stdout(", "#[allow(dead_code)]\nasync fn print_events_to_stdout(")

with open("src/main.rs", "w") as f:
    f.write(content)
