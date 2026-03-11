import re

with open("crates/pi-core/src/agent.rs", "r") as f:
    code = f.read()

code = code.replace(
    "enum Command {",
    "#[allow(dead_code)]\nenum Command {"
)

with open("crates/pi-core/src/agent.rs", "w") as f:
    f.write(code)
