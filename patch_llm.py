import sys
content = open("crates/pi-llm/src/lib.rs").read()

content = content.replace(
    'stream::once(async move { stream.await }).flat_map(|events| stream::iter(events)),',
    'stream::once(stream).flat_map(stream::iter),'
)

open("crates/pi-llm/src/lib.rs", "w").write(content)
