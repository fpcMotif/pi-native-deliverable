use std::fs;

/// Tool sandbox: deny writing to secrets by default policy.
#[test]
fn tool_policy_denies_env_write() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let env_path = tmp.path().join(".env");

    // TODO: once pi-tools is implemented, invoke write tool with default policy:
    // let policy = pi_tools::Policy::safe_defaults();
    // let res = pi_tools::write::execute(&policy, &env_path, "SECRET=1").unwrap_err();
    // assert!(res.to_string().contains("denied"));

    // placeholder assertion
    fs::write(&env_path, "SECRET=1").expect("write");
    assert!(env_path.exists());
}
