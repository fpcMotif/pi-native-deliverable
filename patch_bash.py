import sys
import re

with open("crates/pi-tools/src/bash_test.rs", "r") as f:
    content = f.read()

content = content.replace(
    "fn test_bash_tool_output() {",
    "fn test_bash_tool_output() -> std::result::Result<(), Box<dyn std::error::Error>> {"
)
content = content.replace(
    "let policy = Policy::safe_defaults(std::env::current_dir().unwrap());",
    "let policy = Policy::safe_defaults(std::env::current_dir()?);"
)
content = content.replace(
    """        let res = tool
            .execute(&call, &policy, std::path::Path::new("."))
            .unwrap();""",
    """        let res = tool
            .execute(&call, &policy, std::path::Path::new("."))?;"""
)
content = content.replace(
    """            res.stdout
        );
    }""",
    """            res.stdout
        );
        Ok(())
    }"""
)

content = content.replace(
    "fn test_bash_tool_timeout() {",
    "fn test_bash_tool_timeout() -> std::result::Result<(), Box<dyn std::error::Error>> {"
)
content = content.replace(
    """        let res = tool
            .execute(&call, &policy, std::path::Path::new("."))
            .unwrap();""",
    """        let res = tool
            .execute(&call, &policy, std::path::Path::new("."))?;"""
)
content = content.replace(
    """        assert_eq!(res.error, Some("command timed out".to_string()));
    }""",
    """        assert_eq!(res.error, Some("command timed out".to_string()));
        Ok(())
    }"""
)

with open("crates/pi-tools/src/bash_test.rs", "w") as f:
    f.write(content)
