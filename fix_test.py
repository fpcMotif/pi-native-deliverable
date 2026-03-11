with open("tests/grep_modes.rs", "r") as f:
    content = f.read()

content = content.replace("plain.matches.len() >= 1", "!plain.matches.is_empty()")
content = content.replace("regex.matches.len() >= 1", "!regex.matches.is_empty()")

with open("tests/grep_modes.rs", "w") as f:
    f.write(content)
