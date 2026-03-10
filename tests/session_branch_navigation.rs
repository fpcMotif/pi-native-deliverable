use pi_core::{Agent, AgentConfig};
use pi_llm::MockProvider;
use pi_protocol::protocol_version;
use pi_protocol::rpc::ClientRequest;
use pi_protocol::session::SessionEntryKind;
use pi_search::{SearchService, SearchServiceConfig};
use pi_session::SessionStore;
use pi_tools::{default_registry, Policy};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

async fn build_agent(workspace: &std::path::Path) -> Agent {
    let search_service = SearchService::new(SearchServiceConfig {
        workspace_root: workspace.to_path_buf(),
        ..Default::default()
    })
    .await
    .expect("search service");

    let session_path =
        SessionStore::resolve_session_path(".pi/session.jsonl", workspace).expect("session path");
    let session_store = SessionStore::new(session_path)
        .await
        .expect("session store");

    Agent::new(AgentConfig {
        provider: Arc::new(MockProvider),
        tool_registry: default_registry(search_service),
        tool_policy: Policy::safe_defaults(workspace.to_path_buf()),
        session_store: Arc::new(Mutex::new(session_store)),
        workspace_root: workspace.to_path_buf(),
        default_provider_model: "mock-tool-call".to_string(),
        line_limit: 1024 * 1024,
        extension_host: Arc::new(Mutex::new(pi_ext::RuntimeHost::default())),
    })
    .await
}

#[tokio::test]
async fn branch_navigation_is_append_only() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let agent = build_agent(tmp.path()).await;

    agent
        .handle_request(ClientRequest::Prompt {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            message: "hello".to_string(),
            attachments: None,
        })
        .await
        .expect("prompt request");

    let (anchor, before_ids, before_len) = {
        let store = agent.config.session_store.lock().await;
        let ids: Vec<Uuid> = store
            .log
            .entries
            .iter()
            .map(|entry| entry.entry_id)
            .collect();
        (ids[0], ids.clone(), ids.len())
    };

    agent
        .handle_request(ClientRequest::ForkSession {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: anchor.to_string(),
        })
        .await
        .expect("fork request");

    agent
        .handle_request(ClientRequest::CheckoutBranchHead {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: Some(anchor.to_string()),
        })
        .await
        .expect("checkout branch request");

    let store = agent.config.session_store.lock().await;
    assert!(
        store.log.entries.len() > before_len,
        "navigation actions should append new entries"
    );
    for (idx, original) in before_ids.iter().enumerate() {
        assert_eq!(
            *original, store.log.entries[idx].entry_id,
            "existing entries must remain stable"
        );
    }

    assert!(store
        .log
        .entries
        .iter()
        .any(|entry| matches!(entry.kind, SessionEntryKind::SessionFork { .. })));
    assert!(store.log.entries.iter().any(|entry| {
        matches!(
            &entry.kind,
            SessionEntryKind::SessionMetadata { payload }
                if payload.get("action").and_then(|v| v.as_str()) == Some("checkout_branch_head")
        )
    }));
}

#[tokio::test]
async fn checkout_branch_head_keeps_branch_continuity() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let agent = build_agent(tmp.path()).await;

    agent
        .handle_request(ClientRequest::Prompt {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            message: "first".to_string(),
            attachments: None,
        })
        .await
        .expect("first prompt");

    let anchor = {
        let store = agent.config.session_store.lock().await;
        store.log.entries[0].entry_id
    };

    let fork_events = agent
        .handle_request(ClientRequest::ForkSession {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: anchor.to_string(),
        })
        .await
        .expect("fork request");

    let fork_head = fork_events
        .iter()
        .find_map(|event| match event {
            pi_protocol::ServerEvent::State { payload, .. } => payload
                .get("head")
                .and_then(|value| value.as_str())
                .and_then(|value| Uuid::parse_str(value).ok()),
            _ => None,
        })
        .expect("fork head");

    agent
        .handle_request(ClientRequest::CheckoutBranchHead {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            from_turn_id: Some(anchor.to_string()),
        })
        .await
        .expect("checkout request");

    let store = agent.config.session_store.lock().await;
    let metadata = store
        .log
        .entries
        .iter()
        .rev()
        .find_map(|entry| match &entry.kind {
            SessionEntryKind::SessionMetadata { payload } => Some(payload.clone()),
            _ => None,
        })
        .expect("checkout metadata");

    assert_eq!(
        metadata
            .get("head")
            .and_then(|value| value.as_str())
            .map(str::to_string),
        Some(fork_head.to_string())
    );
}
