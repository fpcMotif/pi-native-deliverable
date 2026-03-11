with open("src/main.rs", "r") as f:
    content = f.read()

content = content.replace(
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

content = content.replace("return std::sync::Arc::new(pi_llm::openai::OpenAIProvider::new(base, key));", "std::sync::Arc::new(pi_llm::openai::OpenAIProvider::new(base, key))")

with open("src/main.rs", "w") as f:
    f.write(content)
