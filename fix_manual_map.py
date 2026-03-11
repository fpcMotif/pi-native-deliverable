import re

with open("src/main.rs", "r") as f:
    content = f.read()

replacement = """    } else {
        trimmed.strip_prefix("/branch-from-turn ").map(|turn_id| ClientRequest::ForkSession {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: turn_id.trim().to_string(),
        })
    };"""

content = re.sub(
    r'    \} else if let Some\(turn_id\) = trimmed\.strip_prefix\("/branch-from-turn "\) \{\n        Some\(ClientRequest::ForkSession \{\n            v: protocol_version\(\),\n            id: Some\(Uuid::new_v4\(\)\.to_string\(\)\),\n            from_turn_id: turn_id\.trim\(\)\.to_string\(\),\n        \}\)\n    \} else \{\n        None\n    \};',
    replacement,
    content
)

with open("src/main.rs", "w") as f:
    f.write(content)
