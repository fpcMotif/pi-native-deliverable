use criterion::{criterion_group, criterion_main, Criterion};
use pi_core::{Agent, AgentConfig};
use pi_llm::MockProvider;
use pi_tools::{default_registry, Policy};
use std::sync::Arc;
use tokio::sync::Mutex;
use pi_session::SessionStore;
use std::path::PathBuf;
use pi_protocol::rpc::ClientRequest;
use uuid::Uuid;
use pi_search::{SearchService, SearchServiceConfig};

fn bench_agent_handle_turn(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let search_config = SearchServiceConfig::default();
    let search_service = rt.block_on(async { SearchService::new(search_config).await.unwrap() });
    let tool_registry = default_registry(search_service.clone());

    let session_store = rt.block_on(async { SessionStore::new(
        PathBuf::from("/tmp/pi_bench_session_store_123"),
    ).await.unwrap() });

    let config = AgentConfig {
        provider: Arc::new(MockProvider),
        tool_registry,
        tool_policy: Policy::safe_defaults(std::path::Path::new("/")),
        session_store: Arc::new(Mutex::new(session_store)),
        workspace_root: PathBuf::from("/"),
        default_provider_model: "test-model".to_string(),
        line_limit: 1000,
    };

    let agent = rt.block_on(async { Agent::new(config).await });

    c.bench_function("agent_handle_turn", |b| {
        b.to_async(&rt).iter(|| async {
            let request = ClientRequest::Prompt {
                v: "1".to_string(),
                id: Some(Uuid::new_v4().to_string()),
                message: "test prompt".to_string(),
                attachments: None,
            };
            let _ = agent.handle_request(request).await.unwrap();
        })
    });
}

criterion_group!(benches, bench_agent_handle_turn);
criterion_main!(benches);
