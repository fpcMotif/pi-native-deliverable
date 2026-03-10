use pi_core::{Agent, AgentConfig};
use pi_llm::MockProvider;
use pi_protocol::{parse_client_request, session::SessionEntryKind};
use pi_session::SessionStore;
use pi_tools::{Policy, ToolRegistry};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::Mutex;

async fn build_agent(workspace: &TempDir) -> (Agent, std::path::PathBuf) {
    let session_path = workspace.path().join(".pi").join("session.jsonl");
    let session_store = SessionStore::new(session_path.clone())
        .await
        .expect("session store");

    let config = AgentConfig {
        provider: Arc::new(MockProvider),
        tool_registry: ToolRegistry::new(),
        tool_policy: Policy::safe_defaults(workspace.path()),
        session_store: Arc::new(Mutex::new(session_store)),
        workspace_root: workspace.path().to_path_buf(),
        default_provider_model: "mock-tool-call".to_string(),
        line_limit: 1024 * 1024,
    };

    (Agent::new(config).await, session_path)
}

#[tokio::test]
async fn usage_accumulation_is_persisted_in_session_log() {
    let workspace = TempDir::new().expect("tmp workspace");
    let (agent, session_path) = build_agent(&workspace).await;

    for id in ["req-1", "req-2"] {
        let request = parse_client_request(&format!(
            r#"{{"v":"1.0.0","type":"prompt","id":"{id}","message":"hello"}}"#
        ))
        .expect("parse request");
        agent.handle_request(request).await.expect("handle prompt");
    }

    let reloaded = SessionStore::load(session_path)
        .await
        .expect("reload session");
    let metadata_entries = reloaded
        .log
        .entries
        .iter()
        .filter_map(|entry| match &entry.kind {
            SessionEntryKind::SessionMetadata { payload } => Some(payload),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(metadata_entries.len(), 2);
    let latest = metadata_entries.last().expect("latest usage metadata");
    assert_eq!(latest["type"], "usage_totals");
    assert_eq!(latest["turn"]["input_tokens"], 12);
    assert_eq!(latest["session"]["input_tokens"], 24);
    assert_eq!(latest["session"]["output_tokens"], 14);
}

#[tokio::test]
async fn session_export_json_includes_token_and_cost_totals() {
    let workspace = TempDir::new().expect("tmp workspace");
    let (agent, _) = build_agent(&workspace).await;

    let request =
        parse_client_request(r#"{"v":"1.0.0","type":"prompt","id":"req-1","message":"hello"}"#)
            .expect("parse request");
    agent.handle_request(request).await.expect("handle prompt");

    let store = agent.config.session_store.lock().await;
    let export = store.to_jsonl_string().expect("export session");

    assert!(export.contains("\"input_tokens\":12"));
    assert!(export.contains("\"output_tokens\":7"));
    assert!(export.contains("\"cost_usd\":0.0019"));
}
