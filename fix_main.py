import re

with open('src/main.rs', 'r') as f:
    content = f.read()

# Fix manual map
old_if_let = """    } else if let Some(turn_id) = trimmed.strip_prefix("/branch-from-turn ") {
        Some(ClientRequest::ForkSession {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: turn_id.trim().to_string(),
        })
    } else {
        None
    };"""

new_if_let = """    } else {
        trimmed.strip_prefix("/branch-from-turn ").map(|turn_id| ClientRequest::ForkSession {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: turn_id.trim().to_string(),
        })
    };"""

content = content.replace(old_if_let, new_if_let)

# Fix needless return
old_return = """                let key = std::env::var("OPENAI_API_KEY").ok();
                return std::sync::Arc::new(pi_llm::openai::OpenAIProvider::new(base, key));
            }"""

new_return = """                let key = std::env::var("OPENAI_API_KEY").ok();
                std::sync::Arc::new(pi_llm::openai::OpenAIProvider::new(base, key))
            }"""

content = content.replace(old_return, new_return)

with open('src/main.rs', 'w') as f:
    f.write(content)
