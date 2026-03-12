import sys

with open("crates/pi-session/src/bin_benchmark.rs", "r") as f:
    content = f.read()

content = content.replace(
    "unwrap_or_else(|_| ());",
    "unwrap_or(());"
)

with open("crates/pi-session/src/bin_benchmark.rs", "w") as f:
    f.write(content)

with open("crates/pi-session/benches/session_load.rs", "r") as f:
    content = f.read()

content = content.replace(
    "unwrap_or_else(|_| ());",
    "unwrap_or(());"
)

with open("crates/pi-session/benches/session_load.rs", "w") as f:
    f.write(content)
