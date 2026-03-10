use pi_ext::{
    load_manifests_from_dir, Capability, ExtensionManifest, ExtensionRuntime, LifecycleAction,
    Policy, RuntimeError,
};
use pi_protocol::session::{SessionEntry, SessionEntryKind};
use pi_session::SessionStore;
use std::collections::HashMap;
use std::fs;
use tempfile::tempdir;

fn write_manifest(dir: &std::path::Path, name: &str, version: &str, caps: Vec<Capability>) {
    let manifest = ExtensionManifest {
        name: name.to_string(),
        version: version.to_string(),
        capabilities: caps,
        entrypoint: "index.js".to_string(),
        metadata: HashMap::new(),
    };
    let path = dir.join(format!("{name}.json"));
    let raw = serde_json::to_string_pretty(&manifest).expect("serialize manifest");
    fs::write(path, raw).expect("write manifest");
}

#[test]
fn extension_load_failure_diagnostics_are_reported() {
    let tmp = tempdir().expect("tempdir");
    write_manifest(tmp.path(), "ok", "1.0.0", vec![Capability::FileRead]);
    fs::write(tmp.path().join("broken.json"), "{ bad json").expect("write bad manifest");

    let report = load_manifests_from_dir(tmp.path());
    assert_eq!(report.loaded.len(), 1);
    assert_eq!(report.diagnostics.len(), 1);
    assert!(report.diagnostics[0].message.contains("parse error"));
}

#[test]
fn capability_gate_is_enforced_at_hostcall_time() {
    let tmp = tempdir().expect("tempdir");
    write_manifest(
        tmp.path(),
        "net_ext",
        "1.0.0",
        vec![Capability::NetworkHttp],
    );

    let mut denied_runtime = ExtensionRuntime::new(Policy::safe(), tmp.path().to_path_buf());
    let _ = denied_runtime.reload();
    let denied = denied_runtime.invoke_hostcall("net_ext", "http_get", Capability::NetworkHttp);
    assert!(matches!(denied, Err(RuntimeError::CapabilityDenied { .. })));

    let mut allowed_runtime = ExtensionRuntime::new(
        Policy::safe().allow(Capability::NetworkHttp),
        tmp.path().to_path_buf(),
    );
    let _ = allowed_runtime.reload();
    let allowed = allowed_runtime
        .invoke_hostcall("net_ext", "http_get", Capability::NetworkHttp)
        .expect("allowed hostcall");
    assert!(allowed.0.allowed);
    assert_eq!(allowed.1.action, LifecycleAction::InvocationAllowed);
}

#[test]
fn hot_reload_emits_reloaded_action() {
    let tmp = tempdir().expect("tempdir");
    write_manifest(tmp.path(), "hot", "1.0.0", vec![Capability::FileRead]);

    let mut runtime = ExtensionRuntime::new(Policy::safe(), tmp.path().to_path_buf());
    let (_report1, events1) = runtime.reload();
    assert!(events1
        .iter()
        .any(|event| event.action == LifecycleAction::Loaded));

    write_manifest(tmp.path(), "hot", "1.1.0", vec![Capability::FileRead]);
    let (_report2, events2) = runtime.reload();
    assert!(events2
        .iter()
        .any(|event| event.action == LifecycleAction::Reloaded && event.manifest == "hot"));
}

#[tokio::test]
async fn extension_events_are_audited_into_session_log() {
    let tmp = tempdir().expect("tempdir");
    write_manifest(tmp.path(), "audit", "1.0.0", vec![Capability::FileRead]);

    let mut runtime = ExtensionRuntime::new(Policy::safe(), tmp.path().to_path_buf());
    let (_report, events) = runtime.reload();

    let session_path = tmp.path().join("session.jsonl");
    let mut store = SessionStore::new(session_path.clone())
        .await
        .expect("session store");

    for event in events {
        let action = match event.action {
            LifecycleAction::Loaded => "loaded",
            LifecycleAction::Reloaded => "reloaded",
            LifecycleAction::Unloaded => "unloaded",
            LifecycleAction::InvocationAllowed => "invocation_allowed",
            LifecycleAction::InvocationDenied => "invocation_denied",
        }
        .to_string();

        store
            .append(SessionEntryKind::ExtensionEvent {
                manifest: event.manifest,
                action,
            })
            .await
            .expect("append extension event");
    }

    let raw = fs::read_to_string(session_path).expect("read session file");
    let parsed: SessionEntry =
        serde_json::from_str(raw.lines().next().expect("one line")).expect("entry parse");
    assert!(matches!(
        parsed.kind,
        SessionEntryKind::ExtensionEvent { .. }
    ));
}
