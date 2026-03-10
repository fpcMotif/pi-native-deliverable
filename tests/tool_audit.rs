use async_trait::async_trait;
use futures::stream::{self, BoxStream};
use pi_core::{Agent, AgentConfig};
use pi_llm::{CompletionRequest, ModelCard, Provider, ProviderEvent};
use pi_protocol::{rpc::ClientRequest, ServerEvent};
use pi_search::{SearchService, SearchServiceConfig};
use pi_session::SessionStore;
use pi_tools::{default_registry, Policy};
use serde_json::json;
use std::sync::Arc;

#[derive(Debug)]
struct DeniedWriteProvider;

#[async_trait]
impl Provider for DeniedWriteProvider {
    fn name(&self) -> &'static str {
        "denied-write-provider"
    }

    async fn list_models(&self) -> pi_llm::Result<Vec<ModelCard>> {
        Ok(vec![ModelCard {
            id: "denied-write-model".to_string(),
            provider: self.name().to_string(),
            context_window: Some(1024),
        }])
    }

    fn stream(
        &self,
        _request: CompletionRequest,
        request_id: Option<String>,
    ) -> BoxStream<'static, ProviderEvent> {
        Box::pin(stream::iter(vec![
            ProviderEvent::ToolCall {
                request_id: request_id.clone(),
                tool_name: "write".to_string(),
                args: json!({"path": ".env", "content": "SECRET=2"}),
            },
            ProviderEvent::Done {
                request_id,
                stop_reason: "complete".to_string(),
            },
        ]))
    }
}

#[tokio::test]
async fn denied_tool_attempt_has_reason_and_audit_log_persists() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().to_path_buf();

    let session_path =
        SessionStore::resolve_session_path(".pi/session.jsonl", &workspace).expect("session path");
    let session_store = SessionStore::new(session_path)
        .await
        .expect("session store");
    let search = SearchService::new(SearchServiceConfig {
        workspace_root: workspace.clone(),
        ..Default::default()
    })
    .await
    .expect("search");

    let agent = Agent::new(AgentConfig {
        provider: Arc::new(DeniedWriteProvider),
        tool_registry: default_registry(search),
        tool_policy: Policy::safe(workspace.clone()),
        session_store: Arc::new(tokio::sync::Mutex::new(session_store)),
        workspace_root: workspace,
        default_provider_model: "denied-write-model".to_string(),
        line_limit: 1024 * 1024,
    })
    .await;

    let events = agent
        .handle_request(ClientRequest::Prompt {
            v: "1.0.0".to_string(),
            id: Some("req-denied".to_string()),
            message: "attempt denied write".to_string(),
            attachments: None,
        })
        .await
        .expect("prompt response");

    let denied = events.iter().find_map(|event| match event {
        ServerEvent::ToolCallError { error, .. } => Some(error.message.clone()),
        _ => None,
    });
    let denied_message = denied.expect("tool call error event");
    assert!(
        denied_message.contains("PT002"),
        "expected rule id in denial message: {denied_message}"
    );

    let state_events = agent
        .handle_request(ClientRequest::GetState {
            v: "1.0.0".to_string(),
            id: Some("req-state".to_string()),
        })
        .await
        .expect("state response");

    let payload = state_events
        .iter()
        .find_map(|event| match event {
            ServerEvent::State { payload, .. } => Some(payload.clone()),
            _ => None,
        })
        .expect("state payload");

    let audit_events = payload
        .get("audit_events")
        .and_then(|value| value.as_array())
        .expect("audit_events array");
    let denied_audit = audit_events
        .iter()
        .find(|item| item.get("outcome").and_then(|value| value.as_str()) == Some("deny"))
        .expect("deny audit event");

    assert_eq!(
        denied_audit.get("rule_id").and_then(|value| value.as_str()),
        Some("PT002")
    );
    assert!(denied_audit
        .get("reason")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .contains("write path blocked"));
}
