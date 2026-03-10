use pi_ext::{Capability, ExtensionLoader, Policy};
use pi_tools::{Policy as ToolPolicy, ToolCall, ToolRegistry};
use serde_json::json;
use std::fs;

fn write_manifest(root: &std::path::Path, required_capability: &str) {
    let ext_dir = root.join("sample");
    fs::create_dir_all(&ext_dir).expect("create ext dir");
    fs::write(
        ext_dir.join("manifest.json"),
        serde_json::to_string_pretty(&json!({
            "name": "sample",
            "version": "0.1.0",
            "capabilities": ["file_read", "network_http"],
            "entrypoint": "sample.js",
            "metadata": {
                "tool_name": "sample_echo",
                "tool_description": "sample extension tool",
                "tool_response": "echo:",
                "required_capability": required_capability
            }
        }))
        .expect("json"),
    )
    .expect("write manifest");
}

#[test]
fn extension_loader_registers_and_invokes_tool_when_policy_allows() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_manifest(temp.path(), "file_read");

    let mut loader = ExtensionLoader::new(temp.path(), Policy::safe().allow(Capability::FileRead));
    let mut registry = ToolRegistry::new();
    let events = loader.initialize(&mut registry).expect("load extensions");
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].action, "load");

    let result = registry
        .execute(
            "sample_echo",
            &ToolCall {
                id: "1".to_string(),
                name: "sample_echo".to_string(),
                args: json!({"input":"ok"}),
            },
            &ToolPolicy::safe_defaults(temp.path()),
            temp.path(),
        )
        .expect("execute extension tool");

    assert_eq!(result.status.as_str(), "ok");
    assert_eq!(result.stdout, "echo:ok");
    assert_eq!(
        result
            .metadata
            .get("extension_manifest")
            .and_then(|v| v.as_str()),
        Some("sample")
    );
}

#[test]
fn extension_loader_enforces_extension_capability_policy() {
    let temp = tempfile::tempdir().expect("tempdir");
    write_manifest(temp.path(), "network_http");

    let mut loader = ExtensionLoader::new(temp.path(), Policy::safe());
    let mut registry = ToolRegistry::new();
    loader.initialize(&mut registry).expect("load extensions");

    let result = registry
        .execute(
            "sample_echo",
            &ToolCall {
                id: "2".to_string(),
                name: "sample_echo".to_string(),
                args: json!({}),
            },
            &ToolPolicy::safe_defaults(temp.path()),
            temp.path(),
        )
        .expect("execute extension tool");

    assert_eq!(result.status.as_str(), "denied");
    assert!(result.error.unwrap_or_default().contains("safe policy"));
}
