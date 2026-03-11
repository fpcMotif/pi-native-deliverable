import re

with open("crates/pi-llm/src/lib.rs", "r") as f:
    code = f.read()

replacement = r"""            Box::pin(
                stream::once(stream).flat_map(stream::iter),
            )"""

code = code.replace(
    "            Box::pin(\n                stream::once(async move { stream.await }).flat_map(|events| stream::iter(events)),\n            )",
    replacement
)

with open("crates/pi-llm/src/lib.rs", "w") as f:
    f.write(code)
