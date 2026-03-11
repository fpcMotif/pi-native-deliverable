import re

with open("src/main.rs", "r") as f:
    code = f.read()

# Fix unused import
code = code.replace(
    "use tokio::io::{self as tokio_io, AsyncBufReadExt, AsyncWriteExt};",
    "use tokio::io::{self as tokio_io, AsyncBufReadExt};"
)

# Fix dead code
code = code.replace(
    "async fn apply_startup_session_controls(_agent: &Agent, _cli: &Cli) {",
    "#[allow(dead_code)]\nasync fn apply_startup_session_controls(_agent: &Agent, _cli: &Cli) {"
)
code = code.replace(
    "async fn print_events_to_stdout(events: &[ServerEvent]) {",
    "#[allow(dead_code)]\nasync fn print_events_to_stdout(events: &[ServerEvent]) {"
)

# Fix needless return
code = code.replace(
    "                return std::sync::Arc::new(pi_llm::openai::OpenAIProvider::new(base, key));",
    "                std::sync::Arc::new(pi_llm::openai::OpenAIProvider::new(base, key))"
)

# Fix manual_map
manual_map_old = """    } else if let Some(turn_id) = trimmed.strip_prefix("/branch-from-turn ") {
        Some(ClientRequest::ForkSession {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: turn_id.trim().to_string(),
        })
    } else {
        None
    };"""

manual_map_new = """    } else {
        trimmed.strip_prefix("/branch-from-turn ").map(|turn_id| ClientRequest::ForkSession {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: turn_id.trim().to_string(),
        })
    };"""

code = code.replace(manual_map_old, manual_map_new)

with open("src/main.rs", "w") as f:
    f.write(code)
