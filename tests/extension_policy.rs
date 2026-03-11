use pi_ext::{explain_policy, Capability, Policy};

#[test]
fn extension_policy_explains_denial() {
    let policy = Policy::safe();
    let decision = policy.check(Capability::NetworkHttp);
    assert!(!decision.allowed);
    assert!(decision.reason.contains("safe policy"));
    assert!(explain_policy(&policy, Capability::NetworkHttp).contains("allowed=false"));
}

#[test]
fn extension_policy_allows_workspace_capability_by_default() {
    let policy = Policy::safe();
    let decision = policy.check(Capability::FileRead);
    assert!(decision.allowed);
    assert_eq!(decision.capability, Capability::FileRead);
}

#[test]
fn extension_policy_can_deny_file_write() {
    let policy = Policy::default().deny(Capability::FileWrite);
    let decision = policy.check(Capability::FileWrite);
    assert!(!decision.allowed);
    assert!(decision.reason.contains("safe policy"));
}
