use super::*;
use serde_json::json;
use std::path::Path;

struct DummyTool;

impl Tool for DummyTool {
    fn name(&self) -> &'static str {
        "dummy"
    }

    fn description(&self) -> &'static str {
        "Dummy tool for testing"
    }

    fn schema(&self) -> serde_json::Value {
        json!({"name": "dummy"})
    }

    fn execute(&self, call: &ToolCall, _policy: &Policy, _cwd: &Path) -> Result<ToolCallResult> {
        let should_fail = call
            .args
            .get("fail")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if should_fail {
            Err(ToolError::Error("dummy error".to_string()))
        } else {
            Ok(ToolCallResult {
                stdout: "success".to_string(),
                status: ToolStatus::Ok,
                error: None,
                truncated: false,
                metadata: std::collections::BTreeMap::new(),
            })
        }
    }
}

#[test]
fn test_execute_with_audit() {
    let mut registry = ToolRegistry::new();
    registry.register(DummyTool);

    let policy = Policy::safe_defaults(std::env::current_dir().unwrap());
    let cwd = std::env::current_dir().unwrap();

    let mut audit = Vec::new();

    // Success case
    let success_call = make_call("dummy", json!({"fail": false}));
    let res = registry.execute_with_audit("dummy", &success_call, &policy, &cwd, &mut audit);

    assert!(res.is_ok());
    assert_eq!(audit.len(), 1);

    let record = &audit[0];
    assert_eq!(record.call_id, success_call.id);
    assert_eq!(record.tool, "dummy");
    assert!(record.allowed);
    assert_eq!(record.rule_id, "tool.allow");
    assert_eq!(record.status, ToolStatus::Ok);
    assert!(record.error.is_none());

    // Error case
    let error_call = make_call("dummy", json!({"fail": true}));
    let res = registry.execute_with_audit("dummy", &error_call, &policy, &cwd, &mut audit);

    assert!(res.is_err());
    assert_eq!(audit.len(), 2);

    let record = &audit[1];
    assert_eq!(record.call_id, error_call.id);
    assert_eq!(record.tool, "dummy");
    assert!(!record.allowed);
    // tool_error_rule_id returns "tool.execution.error" for ToolError::Error
    assert_eq!(record.rule_id, "tool.execution.error");
    assert_eq!(record.status, ToolStatus::Denied);
    assert_eq!(record.error, Some("tool error: dummy error".to_string()));
}
