use pi_ext::{Capability, ExtensionError, RuntimeHost, RuntimeRegistration};
use serde_json::json;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

fn write_manifest(
    path: &std::path::Path,
    name: &str,
    capabilities: &[Capability],
    tool_name: &str,
) {
    let manifest = json!({
        "name": name,
        "version": "0.1.0",
        "entrypoint": "index.js",
        "capabilities": capabilities,
        "tools": [{"name": tool_name, "description": "tool"}],
        "commands": [],
        "event_hooks": [],
        "metadata": {}
    });
    fs::write(
        path,
        serde_json::to_vec_pretty(&manifest).expect("manifest json"),
    )
    .expect("write manifest");
}

#[test]
fn load_extension_and_register_tool() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("sample.json");
    write_manifest(
        &manifest_path,
        "demo-ext",
        &[Capability::ToolRegister],
        "sample_tool",
    );

    let mut host = RuntimeHost::default();
    host.load_extension_manifest(&manifest_path)
        .expect("load extension");

    let tools = host.tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "sample_tool");
    assert_eq!(tools[0].extension, "demo-ext");
}

#[test]
fn deny_unauthorized_capability_hostcall() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let manifest_path = tmp.path().join("sample.json");
    write_manifest(
        &manifest_path,
        "demo-ext",
        &[Capability::CommandRegister],
        "sample_tool",
    );

    let mut host = RuntimeHost::default();
    host.load_extension_manifest(&manifest_path)
        .expect_err("manifest tool registration should fail without capability");

    // Load an extension that omits tool registration in manifest, then call host API directly.
    let manifest = json!({
        "name": "manual-ext",
        "version": "0.1.0",
        "entrypoint": "index.js",
        "capabilities": ["command_register"],
        "tools": [],
        "commands": [],
        "event_hooks": [],
        "metadata": {}
    });
    let manual_manifest_path = tmp.path().join("manual.json");
    fs::write(
        &manual_manifest_path,
        serde_json::to_vec_pretty(&manifest).expect("manifest"),
    )
    .expect("write manual");
    host.load_extension_manifest(&manual_manifest_path)
        .expect("load manual ext");

    let err = host
        .register_tool(
            "manual-ext",
            RuntimeRegistration {
                name: "new_tool".to_string(),
                description: "desc".to_string(),
            },
        )
        .expect_err("missing tool_register capability");

    assert!(matches!(
        err,
        ExtensionError::CapabilityDenied { extension, .. } if extension == "manual-ext"
    ));
}

#[test]
fn hot_reload_behavior_in_interactive_session() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path();
    let extension_dir = workspace.join(".pi/extensions");
    fs::create_dir_all(&extension_dir).expect("create extension dir");

    let manifest_path = extension_dir.join("reloadable.json");
    write_manifest(
        &manifest_path,
        "reloadable",
        &[Capability::ToolRegister],
        "tool_v1",
    );

    let mut child = Command::new(env!("CARGO_BIN_EXE_pi"))
        .args([
            "--mode",
            "interactive",
            "--provider",
            "mock",
            "--workspace",
            workspace.to_str().expect("workspace str"),
            "--extensions-dir",
            extension_dir.to_str().expect("extension dir str"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("spawn interactive");

    write_manifest(
        &manifest_path,
        "reloadable",
        &[Capability::ToolRegister],
        "tool_v2",
    );

    let stdin = child.stdin.as_mut().expect("stdin");
    stdin.write_all(b"/reload\n/exit\n").expect("send commands");

    let output = child.wait_with_output().expect("wait output");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("extensions reloaded: 1"),
        "stdout was: {stdout}"
    );
    assert!(output.status.success(), "process should exit successfully");
}
