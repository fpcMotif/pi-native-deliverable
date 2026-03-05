use pi_ext::{explain_policy, Policy, Capability};

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

#[test]
fn extension_policy_can_allow_network_http() {
    let policy = Policy::safe().allow(Capability::NetworkHttp);
    let decision = policy.check(Capability::NetworkHttp);
    assert!(decision.allowed);
    assert!(decision.reason.contains("allowed by policy"));
}

#[test]
fn extension_policy_allow_is_idempotent() {
    let policy = Policy::safe().allow(Capability::FileRead).allow(Capability::FileRead);
    let decision = policy.check(Capability::FileRead);
    assert!(decision.allowed);
    assert!(decision.reason.contains("allowed by policy"));
}

#[test]
fn extension_policy_allow_does_not_affect_other_capabilities() {
    let policy = Policy::safe().allow(Capability::NetworkHttp);

    // NetworkHttp should be allowed
    assert!(policy.check(Capability::NetworkHttp).allowed);

    // Bash should still be allowed (default)
    assert!(policy.check(Capability::Bash).allowed);

    // FileRead should still be allowed (default)
    assert!(policy.check(Capability::FileRead).allowed);
}

#[test]
fn extension_policy_deny_allow_interaction() {
    // deny then allow should be allowed
    let policy = Policy::default().deny(Capability::FileRead).allow(Capability::FileRead);
    assert!(policy.check(Capability::FileRead).allowed);

    // allow then deny should be denied
    let policy = Policy::default().allow(Capability::NetworkHttp).deny(Capability::NetworkHttp);
    assert!(!policy.check(Capability::NetworkHttp).allowed);
}
