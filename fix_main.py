import re

with open('src/main.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'use tokio::io::{self as tokio_io, AsyncBufReadExt, AsyncWriteExt};',
    'use tokio::io::{self as tokio_io, AsyncBufReadExt};'
)
content = content.replace('async fn apply_startup_session_controls', '#[allow(dead_code)]\nasync fn apply_startup_session_controls')
content = content.replace('async fn print_events_to_stdout', '#[allow(dead_code)]\nasync fn print_events_to_stdout')

content = re.sub(
    r'println!\(\n            "\{\}",\n            "\{\\"error\\":\\"protocol-schema feature is disabled\\"}"\n        \);',
    'println!("{{\\"error\\":\\"protocol-schema feature is disabled\\"}}");',
    content
)

content = content.replace(
    'return std::sync::Arc::new(pi_llm::openai::OpenAIProvider::new(base, key));',
    'std::sync::Arc::new(pi_llm::openai::OpenAIProvider::new(base, key))'
)

with open('src/main.rs', 'w') as f:
    f.write(content)
