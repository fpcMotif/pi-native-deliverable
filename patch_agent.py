import sys
content = open("crates/pi-core/src/agent.rs").read()

content = content.replace("enum Command {", "#[allow(dead_code)]\nenum Command {")

open("crates/pi-core/src/agent.rs", "w").write(content)
