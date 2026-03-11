import re

with open('crates/pi-llm/src/lib.rs', 'r') as f:
    content = f.read()

content = content.replace(
    'stream::once(async move { stream.await }).flat_map(|events| stream::iter(events))',
    'stream::once(stream).flat_map(stream::iter)'
)

with open('crates/pi-llm/src/lib.rs', 'w') as f:
    f.write(content)
