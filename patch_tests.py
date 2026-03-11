import sys
import re

content_main = open("src/main.rs").read()

content_main = content_main.replace(
    "use tokio::io::{self as tokio_io, AsyncBufReadExt, AsyncWriteExt};",
    "use tokio::io::{self as tokio_io, AsyncBufReadExt};"
)

content_main = content_main.replace("async fn apply_startup_session_controls", "#[allow(dead_code)]\nasync fn apply_startup_session_controls")
content_main = content_main.replace("async fn print_events_to_stdout", "#[allow(dead_code)]\nasync fn print_events_to_stdout")

content_main = content_main.replace(
    """    } else if let Some(turn_id) = trimmed.strip_prefix("/branch-from-turn ") {
        Some(ClientRequest::ForkSession {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: turn_id.trim().to_string(),
        })
    } else {
        None
    };""",
    """    } else {
        trimmed.strip_prefix("/branch-from-turn ").map(|turn_id| ClientRequest::ForkSession {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: turn_id.trim().to_string(),
        })
    };"""
)

content_main = content_main.replace(
    "                return std::sync::Arc::new(pi_llm::openai::OpenAIProvider::new(base, key));",
    "                std::sync::Arc::new(pi_llm::openai::OpenAIProvider::new(base, key))"
)

open("src/main.rs", "w").write(content_main)

content_tests = open("tests/grep_modes.rs").read()
content_tests = content_tests.replace("plain.matches.len() >= 1", "!plain.matches.is_empty()")
content_tests = content_tests.replace("regex.matches.len() >= 1", "!regex.matches.is_empty()")
open("tests/grep_modes.rs", "w").write(content_tests)
