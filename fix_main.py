import re

with open('src/main.rs', 'r') as f:
    content = f.read()

# Fix unused methods
content = content.replace("async fn apply_startup_session_controls(_agent: &Agent, _cli: &Cli) {", "#[allow(dead_code)]\nasync fn apply_startup_session_controls(_agent: &Agent, _cli: &Cli) {")
content = content.replace("async fn print_events_to_stdout(events: &[ServerEvent]) {", "#[allow(dead_code)]\nasync fn print_events_to_stdout(events: &[ServerEvent]) {")

# Fix manual_map
manual_map_pattern = r'\} else if let Some\(turn_id\) = trimmed\.strip_prefix\("/branch-from-turn "\) \{\s*Some\(ClientRequest::ForkSession \{\s*v: protocol_version\(\),\s*id: Some\(Uuid::new_v4\(\)\.to_string\(\)\),\s*from_turn_id: turn_id\.trim\(\)\.to_string\(\),\s*\}\)\s*\} else \{\s*None\s*\}'
manual_map_replacement = r"""} else {
        trimmed.strip_prefix("/branch-from-turn ").map(|turn_id| ClientRequest::ForkSession {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: turn_id.trim().to_string(),
        })
    }"""
content = re.sub(manual_map_pattern, manual_map_replacement, content)

with open('src/main.rs', 'w') as f:
    f.write(content)

with open('crates/pi-search/src/lib.rs', 'r') as f:
    content = f.read()

# Move tests to bottom to fix items after test module
tests_module_pattern = r'#\[cfg\(test\)\]\nmod tests \{.*?\n\}\n'
tests_match = re.search(tests_module_pattern, content, flags=re.DOTALL)
if tests_match:
    tests_content = tests_match.group(0)
    content = content[:tests_match.start()] + content[tests_match.end():]
    content += '\n' + tests_content

with open('crates/pi-search/src/lib.rs', 'w') as f:
    f.write(content)
