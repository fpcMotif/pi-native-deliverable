use std::fs;

#[test]
fn tool_calls_emit_audit_records_for_allow_and_deny() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().to_path_buf();

    let allowed_path = workspace.join("ok.txt");
    fs::write(&allowed_path, "hello\n").expect("seed file");

    let mut audit_log = Vec::new();

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let service = pi_search::SearchService::new(pi_search::SearchServiceConfig {
            workspace_root: workspace.clone(),
            ..Default::default()
        })
        .await
        .unwrap();

        let registry = pi_tools::default_registry(service.clone());
        let policy = pi_tools::Policy::safe_defaults(&workspace);
        let cwd = workspace.as_path();

        let allowed_call = pi_tools::make_call("read", serde_json::json!({"path": "ok.txt"}));
        let allowed_result = registry
            .execute_with_audit(
                &allowed_call.name,
                &allowed_call,
                &policy,
                cwd,
                &mut audit_log,
            )
            .expect("allowed call should succeed");

        assert_eq!(allowed_result.status, pi_tools::ToolStatus::Ok);
        assert_eq!(audit_log.len(), 1);

        let allowed_record = &audit_log[0];
        assert!(allowed_record.allowed);
        assert_eq!(allowed_record.tool, "read");
        assert_eq!(allowed_record.call_id, allowed_call.id);
        assert_eq!(allowed_record.status, pi_tools::ToolStatus::Ok);
        assert_eq!(allowed_record.rule_id, "tool.allow");
        assert!(allowed_record.error.is_none());

        let denied_call = pi_tools::make_call(
            "write",
            serde_json::json!({"path": "../outside.txt", "content": "secret"}),
        );
        assert!(registry
            .execute_with_audit(
                &denied_call.name,
                &denied_call,
                &policy,
                cwd,
                &mut audit_log,
            )
            .is_err());

        assert_eq!(audit_log.len(), 2);
        let denied_record = &audit_log[1];
        assert!(!denied_record.allowed);
        assert_eq!(denied_record.tool, "write");
        assert_eq!(denied_record.status, pi_tools::ToolStatus::Denied);
        assert_eq!(denied_record.rule_id, "tool.policy.denied");
        assert!(denied_record.error.is_some());

        let invalid_call = pi_tools::make_call("read", serde_json::json!({}));
        assert!(registry
            .execute_with_audit(
                &invalid_call.name,
                &invalid_call,
                &policy,
                cwd,
                &mut audit_log,
            )
            .is_err());

        let invalid_record = &audit_log[2];
        assert!(!invalid_record.allowed);
        assert_eq!(invalid_record.rule_id, "tool.input.invalid");
    });
}
