use pi_tools::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

struct MockSuccessTool;

impl Tool for MockSuccessTool {
    fn name(&self) -> &'static str {
        "mock_success"
    }

    fn description(&self) -> &'static str {
        "A mock tool that always succeeds"
    }

    fn schema(&self) -> Value {
        json!({
            "name": "mock_success",
            "type": "object",
            "properties": {}
        })
    }

    fn execute(&self, _call: &ToolCall, _policy: &Policy, _cwd: &Path) -> Result<ToolCallResult> {
        Ok(ToolCallResult {
            stdout: "success output".to_string(),
            status: ToolStatus::Ok,
            error: None,
            truncated: false,
            metadata: BTreeMap::new(),
        })
    }
}

struct MockFailTool;

impl Tool for MockFailTool {
    fn name(&self) -> &'static str {
        "mock_fail"
    }

    fn description(&self) -> &'static str {
        "A mock tool that always fails"
    }

    fn schema(&self) -> Value {
        json!({
            "name": "mock_fail",
            "type": "object",
            "properties": {}
        })
    }

    fn execute(&self, _call: &ToolCall, _policy: &Policy, _cwd: &Path) -> Result<ToolCallResult> {
        Err(ToolError::Denied("mock denied".to_string()))
    }
}

#[test]
fn test_execute_with_audit_success() {
    let mut registry = ToolRegistry::new();
    registry.register(MockSuccessTool);

    let call = make_call("mock_success", json!({}));
    let policy = Policy::safe("/tmp");
    let cwd = PathBuf::from("/tmp");
    let mut audit = Vec::new();

    let result = registry.execute_with_audit("mock_success", &call, &policy, &cwd, &mut audit);

    assert!(result.is_ok());
    let res = result.unwrap();
    assert_eq!(res.stdout, "success output");
    assert_eq!(res.status, ToolStatus::Ok);

    assert_eq!(audit.len(), 1);
    let record = &audit[0];
    assert_eq!(record.call_id, call.id);
    assert_eq!(record.tool, "mock_success");
    assert!(record.allowed);
    assert_eq!(record.rule_id, "tool.allow");
    assert_eq!(record.status, ToolStatus::Ok);
    assert!(record.error.is_none());
}

#[test]
fn test_execute_with_audit_failure() {
    let mut registry = ToolRegistry::new();
    registry.register(MockFailTool);

    let call = make_call("mock_fail", json!({}));
    let policy = Policy::safe("/tmp");
    let cwd = PathBuf::from("/tmp");
    let mut audit = Vec::new();

    let result = registry.execute_with_audit("mock_fail", &call, &policy, &cwd, &mut audit);

    assert!(result.is_err());

    assert_eq!(audit.len(), 1);
    let record = &audit[0];
    assert_eq!(record.call_id, call.id);
    assert_eq!(record.tool, "mock_fail");
    assert!(!record.allowed);
    assert_eq!(record.rule_id, "tool.policy.denied");
    assert_eq!(record.status, ToolStatus::Denied);
    assert_eq!(record.error.as_deref(), Some("tool denied: mock denied"));
}

#[test]
fn test_execute_with_audit_not_found() {
    let registry = ToolRegistry::new();

    let call = make_call("nonexistent_tool", json!({}));
    let policy = Policy::safe("/tmp");
    let cwd = PathBuf::from("/tmp");
    let mut audit = Vec::new();

    let result = registry.execute_with_audit("nonexistent_tool", &call, &policy, &cwd, &mut audit);

    assert!(result.is_err());

    assert_eq!(audit.len(), 1);
    let record = &audit[0];
    assert_eq!(record.call_id, call.id);
    assert_eq!(record.tool, "nonexistent_tool");
    assert!(!record.allowed);
    assert_eq!(record.rule_id, "tool.missing");
    assert_eq!(record.status, ToolStatus::Denied);
    assert!(record.error.as_ref().unwrap().contains("not found"));
}
