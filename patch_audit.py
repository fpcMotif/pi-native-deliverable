import sys

with open("crates/pi-tools/tests/audit_tests.rs", "r") as f:
    content = f.read()

content = content.replace(
    "fn test_execute_with_audit_success() {",
    "fn test_execute_with_audit_success() -> std::result::Result<(), Box<dyn std::error::Error>> {"
)
content = content.replace(
    """    assert!(result.is_ok());
    let res = result.unwrap();""",
    """    assert!(result.is_ok());
    let res = result?;"""
)
content = content.replace(
    """    assert!(record.error.is_none());
}""",
    """    assert!(record.error.is_none());
    Ok(())
}"""
)


content = content.replace(
    "fn test_execute_with_audit_not_found() {",
    "fn test_execute_with_audit_not_found() -> std::result::Result<(), Box<dyn std::error::Error>> {"
)
content = content.replace(
    """    assert_eq!(record.status, ToolStatus::Denied);
    assert!(record.error.as_ref().unwrap().contains("not found"));
}""",
    """    assert_eq!(record.status, ToolStatus::Denied);
    let error_msg = record.error.as_ref().ok_or("Expected error message")?;
    assert!(error_msg.contains("not found"));
    Ok(())
}"""
)

with open("crates/pi-tools/tests/audit_tests.rs", "w") as f:
    f.write(content)
