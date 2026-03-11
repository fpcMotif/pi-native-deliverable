import re

with open('crates/pi-core/src/agent.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'enum Command {',
    '#[allow(dead_code)]\nenum Command {'
)

with open('crates/pi-core/src/agent.rs', 'w') as f:
    f.write(content)

with open('src/main.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'use tokio::io::{self as tokio_io, AsyncBufReadExt, AsyncWriteExt};',
    'use tokio::io::{self as tokio_io, AsyncBufReadExt};'
)

content = content.replace(
    'async fn apply_startup_session_controls',
    '#[allow(dead_code)]\nasync fn apply_startup_session_controls'
)

content = content.replace(
    'async fn print_events_to_stdout',
    '#[allow(dead_code)]\nasync fn print_events_to_stdout'
)

with open('src/main.rs', 'w') as f:
    f.write(content)
