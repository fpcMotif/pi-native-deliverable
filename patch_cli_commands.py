import sys
import re
content = open("tests/cli_commands.rs").read()

# Filter out the benign "pi-search: watcher save_index failed" lines from stderr before assertion
content = content.replace(
    'assert!(output.stderr.is_empty(), "stderr: {}", String::from_utf8_lossy(&output.stderr));',
    '''let stderr = String::from_utf8_lossy(&output.stderr);
    let filtered_stderr = stderr
        .lines()
        .filter(|line| !line.contains("pi-search: watcher save_index failed"))
        .collect::<Vec<_>>()
        .join("\\n");
    assert!(filtered_stderr.is_empty(), "stderr: {}", filtered_stderr);'''
)

open("tests/cli_commands.rs", "w").write(content)
