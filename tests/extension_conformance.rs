use pi_ext::{Capability, ExtensionManifest, ExtensionRuntime, Host, HostCall, Policy};
use serde_json::json;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

struct MockHost;

impl Host for MockHost {
    fn call(&self, _call: HostCall) -> pi_ext::Result<serde_json::Value> {
        Ok(json!({"ok": true}))
    }
}

fn write_extension(root: &std::path::Path, name: &str, tools: &[&str], caps: &[Capability]) {
    let dir = root.join(name);
    fs::create_dir_all(&dir).expect("ext dir");
    let manifest = ExtensionManifest {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        capabilities: caps.to_vec(),
        tools: tools.iter().map(|v| v.to_string()).collect(),
        commands: vec!["/ext-command".to_string()],
        entrypoint: "index.wasm".to_string(),
        metadata: HashMap::new(),
    };
    fs::write(
        dir.join("extension.json"),
        serde_json::to_string_pretty(&manifest).expect("json"),
    )
    .expect("write manifest");
}

#[test]
fn extension_runtime_load_and_hot_reload() {
    let tmp = tempfile::tempdir().expect("tmp");
    let root = tmp.path().join("extensions");
    fs::create_dir_all(&root).expect("root");

    write_extension(&root, "first", &["tool.one"], &[Capability::FileRead]);

    let mut rt = ExtensionRuntime::new(Policy::safe());
    let count = rt
        .load_from_roots(&[PathBuf::from(&root)])
        .expect("load extensions");
    assert_eq!(count, 1);
    assert!(rt.tool_names().contains(&"tool.one".to_string()));

    write_extension(&root, "second", &["tool.two"], &[Capability::FileRead]);
    let count = rt.reload(&[PathBuf::from(&root)]).expect("reload");
    assert_eq!(count, 2);
    assert!(rt.tool_names().contains(&"tool.two".to_string()));
}

#[test]
fn extension_runtime_denies_hostcall_without_capability() {
    let mut rt = ExtensionRuntime::new(Policy::safe());
    rt.register(
        ExtensionManifest {
            name: "no-net".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![Capability::FileRead],
            tools: vec![],
            commands: vec![],
            entrypoint: "index.wasm".to_string(),
            metadata: HashMap::new(),
        },
        PathBuf::from("/tmp/no-net"),
    );

    let err = rt
        .invoke_hostcall(
            "no-net",
            HostCall::HttpGet {
                url: "https://example.com".to_string(),
            },
            &MockHost,
        )
        .expect_err("network call should be denied");
    assert!(err.to_string().contains("missing capability"));
}
