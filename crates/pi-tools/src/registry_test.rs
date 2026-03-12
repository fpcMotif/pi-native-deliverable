#[cfg(test)]
mod tests {
    use crate::*;
    use serde_json::json;
    use std::path::{Path, PathBuf};

    struct SuccessTool;
    impl Tool for SuccessTool {
        fn name(&self) -> &'static str {
            "success_tool"
        }
        fn description(&self) -> &'static str {
            "A success tool"
        }
        fn schema(&self) -> serde_json::Value {
            json!({})
        }
        fn execute(
            &self,
            _call: &ToolCall,
            _policy: &Policy,
            _cwd: &Path,
        ) -> Result<ToolCallResult> {
            Ok(ToolCallResult {
                stdout: "success".to_string(),
                status: ToolStatus::Ok,
                error: None,
                truncated: false,
                metadata: Default::default(),
            })
        }
    }

    struct FailTool;
    impl Tool for FailTool {
        fn name(&self) -> &'static str {
            "fail_tool"
        }
        fn description(&self) -> &'static str {
            "A fail tool"
        }
        fn schema(&self) -> serde_json::Value {
            json!({})
        }
        fn execute(
            &self,
            _call: &ToolCall,
            _policy: &Policy,
            _cwd: &Path,
        ) -> Result<ToolCallResult> {
            Err(ToolError::denied("fail_tool error"))
        }
    }

    #[test]
    fn test_execute_with_audit() {
        let mut registry = ToolRegistry::new();
        registry.register(SuccessTool);
        registry.register(FailTool);

        let policy = Policy {
            preset: PolicyPreset::Permissive,
            workspace_root: PathBuf::from("/"),
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
            command_timeout_ms: 1000,
            max_file_size: 1024,
            deny_write_paths: vec![],
        };

        let cwd = PathBuf::from("/");
        let mut audit = Vec::new();

        // Test success tool
        let success_call = ToolCall {
            id: "call-1".to_string(),
            name: "success_tool".to_string(),
            args: json!({}),
        };

        let result =
            registry.execute_with_audit("success_tool", &success_call, &policy, &cwd, &mut audit);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().stdout, "success");

        assert_eq!(audit.len(), 1);
        let record = &audit[0];
        assert_eq!(record.call_id, "call-1");
        assert_eq!(record.tool, "success_tool");
        assert!(record.allowed);
        assert_eq!(record.rule_id, "tool.allow");
        assert_eq!(record.status, ToolStatus::Ok);
        assert_eq!(record.error, None);

        // Test fail tool
        let fail_call = ToolCall {
            id: "call-2".to_string(),
            name: "fail_tool".to_string(),
            args: json!({}),
        };

        let result =
            registry.execute_with_audit("fail_tool", &fail_call, &policy, &cwd, &mut audit);
        assert!(result.is_err()); // execute_with_audit returns the original Result

        assert_eq!(audit.len(), 2);
        let record = &audit[1];
        assert_eq!(record.call_id, "call-2");
        assert_eq!(record.tool, "fail_tool");
        assert!(!record.allowed);
        // The rule_id should be returned by tool_error_rule_id, let's see what it is
        // We know from error mapping that ToolError::denied usually has rule_id "tool.policy.denied"
        assert_eq!(record.rule_id, "tool.policy.denied");
        assert_eq!(record.status, ToolStatus::Denied);
        assert_eq!(
            record.error.as_deref(),
            Some("tool denied: fail_tool error")
        );
    }
}
