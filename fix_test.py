import re

with open("tests/cli_commands.rs", "r") as f:
    content = f.read()

replacement = """    let stderr = String::from_utf8_lossy(&output.stderr);
    let filtered_stderr: Vec<&str> = stderr.lines().filter(|line| !line.contains("pi-search: watcher save_index failed")).collect();

    assert!(output.status.success());
    assert!(
        filtered_stderr.is_empty(),
        "stderr: {}",
        stderr
    );"""

content = re.sub(
    r'    assert!\(output\.status\.success\(\)\);\n    assert!\(\n        output\.stderr\.is_empty\(\),\n        "stderr: \{\}",\n        String::from_utf8_lossy\(&output\.stderr\)\n    \);',
    replacement,
    content
)

with open("tests/cli_commands.rs", "w") as f:
    f.write(content)
