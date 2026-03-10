#![forbid(unsafe_code)]

use futures::StreamExt;
use pi_ext::RuntimeHost;
use pi_llm::{CompletionMessage, CompletionRequest, Provider, ProviderEvent};
use pi_protocol::rpc::ClientRequest;
use pi_protocol::{make_error_event, protocol_version, ProtocolErrorPayload, ServerEvent};
use pi_session::SessionStore;
use pi_tools::{ToolCall, ToolCallResult, ToolRegistry};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, AgentError>;

pub struct AgentConfig {
    pub provider: Arc<dyn Provider>,
    pub tool_registry: ToolRegistry,
    pub tool_policy: pi_tools::Policy,
    pub session_store: Arc<Mutex<SessionStore>>,
    pub workspace_root: PathBuf,
    pub default_provider_model: String,
    pub line_limit: usize,
    pub extension_host: Arc<Mutex<RuntimeHost>>,
}

pub struct Agent {
    pub config: AgentConfig,
    state: Arc<Mutex<RunState>>,
    abort_tx: mpsc::Sender<()>,
    abort_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<()>>>,
}

#[derive(Debug)]
pub struct CommandBus {
    sender: mpsc::Sender<ClientRequest>,
}

impl CommandBus {
    pub fn sender(&self) -> mpsc::Sender<ClientRequest> {
        self.sender.clone()
    }
}

#[derive(Debug, Clone)]
pub enum RunState {
    Idle,
    Running,
    Aborting,
}

#[derive(Debug)]
pub enum AgentError {
    Provider(String),
    Session(String),
    Tool(String),
    State(String),
}

/// Accumulated token usage and cost for a single turn or entire session.
///
/// Uses `u64` for token counts (safe up to 18 quintillion tokens).
/// Uses `f64` for cost accumulation — acceptable for display purposes but
/// not suitable for financial accounting due to floating-point drift.
#[derive(Debug, Default, Clone, Copy)]
struct UsageTotals {
    input_tokens: u64,
    output_tokens: u64,
    cached_tokens: u64,
    /// Approximate cost in USD. Accumulated via `f64` addition — adequate for
    /// display but subject to floating-point drift over many additions.
    cost_usd: f64,
}

impl UsageTotals {
    fn add_usage(
        &mut self,
        input_tokens: u32,
        output_tokens: u32,
        cached_tokens: u32,
        cost_usd: f64,
    ) {
        self.input_tokens += input_tokens as u64;
        self.output_tokens += output_tokens as u64;
        self.cached_tokens += cached_tokens as u64;
        self.cost_usd += cost_usd;
    }
}

impl std::fmt::Display for AgentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Provider(value) => write!(f, "{value}"),
            Self::Session(value) => write!(f, "{value}"),
            Self::Tool(value) => write!(f, "{value}"),
            Self::State(value) => write!(f, "{value}"),
        }
    }
}

impl Agent {
    pub async fn new(config: AgentConfig) -> Self {
        let (sender, receiver) = mpsc::channel(16);
        Self {
            config,
            state: Arc::new(Mutex::new(RunState::Idle)),
            abort_tx: sender,
            abort_rx: Arc::new(tokio::sync::Mutex::new(receiver)),
        }
    }

    pub async fn run_command_bus(self: Arc<Self>) -> CommandBus {
        let (sender, mut receiver) = mpsc::channel(16);
        let runner = self.clone();

        tokio::spawn(async move {
            while let Some(command) = receiver.recv().await {
                match command {
                    Command::Prompt(request)
                    | Command::Steer(request)
                    | Command::FollowUp(request) => {
                        let _ = runner.handle_request(request).await;
                    }
                    Command::Abort => {
                        let _ = runner.request_abort().await;
                    }
                    Command::GetState => {
                        let _ = runner.current_state_payload().await;
                    }
                    Command::Compact => {
                        let _ = runner.compact_session().await;
                    }
                    Command::NewSession => {
                        let _ = runner.new_session().await;
                    }
                }
            }
        });

        CommandBus { sender }
    }

    async fn current_state_payload(&self) -> serde_json::Value {
        let store = self.config.session_store.lock().await;
        json!({
            "head": store.current_head().await,
            "entries": store.log.entries.len(),
            "roots": store.load_tree().0.len(),
            "request_id": Uuid::new_v4().to_string(),
        })
    }

    pub async fn handle_request(&self, request: ClientRequest) -> Result<Vec<ServerEvent>> {
        let request_id = request.request_id().map(str::to_string);
        let response = match request {
            ClientRequest::Prompt { .. }
            | ClientRequest::Steer { .. }
            | ClientRequest::FollowUp { .. } => self.handle_turn(request).await,
            ClientRequest::Abort { .. } => {
                self.request_abort().await;
                Ok(vec![ServerEvent::State {
                    v: protocol_version(),
                    id: Some(Uuid::new_v4().to_string()),
                    request_id,
                    payload: json!({"state":"aborting"}),
                }])
            }
            ClientRequest::GetState { .. } => Ok(vec![ServerEvent::State {
                v: protocol_version(),
                id: Some(Uuid::new_v4().to_string()),
                request_id,
                payload: self.current_state_payload().await,
            }]),
            ClientRequest::Compact { .. } => {
                self.compact_session().await?;
                Ok(vec![ServerEvent::State {
                    v: protocol_version(),
                    id: Some(Uuid::new_v4().to_string()),
                    request_id,
                    payload: json!({"state": "compaction_started"}),
                }])
            }
            ClientRequest::NewSession { .. } => Ok(self.new_session().await?),
            ClientRequest::SelectSessionPath { path, .. } => {
                Ok(self.select_session_path(path).await?)
            }
            ClientRequest::ForkSession { from_turn_id, .. } => {
                Ok(self.fork_session(from_turn_id).await?)
            }
            ClientRequest::CheckoutBranchHead { from_turn_id, .. } => {
                Ok(self.checkout_branch_head(from_turn_id).await?)
            }
            ClientRequest::Unknown { request_type, .. } => Ok(vec![make_error_event(
                "unsupported_message_type",
                format!("unsupported request type: {request_type}"),
                request_id,
            )]),
        };

        response
    }

    pub async fn reload_extensions(&self) -> Result<usize> {
        let mut host = self.config.extension_host.lock().await;
        host.reload_all()
            .map_err(|err| AgentError::State(err.to_string()))
    }

    async fn new_session(&self) -> Result<Vec<ServerEvent>> {
        {
            let mut store = self.config.session_store.lock().await;
            store
                .reset()
                .map_err(|err| AgentError::Session(err.to_string()))?;
        }

        Ok(vec![ServerEvent::State {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            request_id: None,
            payload: json!({"state": "new_session"}),
        }])
    }

    async fn compact_session(&self) -> Result<()> {
        let mut store = self.config.session_store.lock().await;
        store
            .compact(None)
            .await
            .map(|_| ())
            .map_err(|err| AgentError::Session(err.to_string()))
    }

    async fn select_session_path(&self, path: String) -> Result<Vec<ServerEvent>> {
        let resolved = SessionStore::resolve_session_path(&path, &self.config.workspace_root)
            .map_err(|err| AgentError::Session(err.to_string()))?;
        let mut new_store = SessionStore::new(resolved.clone())
            .await
            .map_err(|err| AgentError::Session(err.to_string()))?;
        new_store
            .append(pi_protocol::session::SessionEntryKind::SessionMetadata {
                payload: json!({"action":"select_session_path", "path": resolved}),
            })
            .await
            .map_err(|err| AgentError::Session(err.to_string()))?;
        {
            let mut store = self.config.session_store.lock().await;
            *store = new_store;
        }

        Ok(vec![ServerEvent::State {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            request_id: None,
            payload: json!({"state": "session_path_selected", "path": path}),
        }])
    }

    async fn fork_session(&self, from_turn_id: String) -> Result<Vec<ServerEvent>> {
        let from_entry_id = Uuid::parse_str(&from_turn_id)
            .map_err(|err| AgentError::State(format!("invalid turn id: {err}")))?;
        let mut store = self.config.session_store.lock().await;
        let head = store
            .branch_from(from_entry_id)
            .await
            .map_err(|err| AgentError::Session(err.to_string()))?;
        let _ = store.checkout(head).await;

        Ok(vec![ServerEvent::State {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            request_id: None,
            payload: json!({"state": "session_forked", "from_turn_id": from_turn_id, "head": head}),
        }])
    }

    async fn checkout_branch_head(&self, from_turn_id: Option<String>) -> Result<Vec<ServerEvent>> {
        let mut store = self.config.session_store.lock().await;
        let branch_anchor = from_turn_id.clone();
        let target = if let Some(anchor_id) = branch_anchor.as_deref() {
            let root = Uuid::parse_str(anchor_id)
                .map_err(|err| AgentError::State(format!("invalid turn id: {err}")))?;
            resolve_branch_head(&store.log.entries, store.load_tree().1, root)
                .ok_or_else(|| AgentError::Session(format!("entry {anchor_id} not found")))?
        } else {
            store
                .current_head()
                .await
                .ok_or_else(|| AgentError::Session("cannot continue empty session".to_string()))?
        };

        if !store.checkout(target).await {
            return Err(AgentError::Session(format!("entry {target} not found")));
        }

        store
            .append(pi_protocol::session::SessionEntryKind::SessionMetadata {
                payload: json!({"action":"checkout_branch_head", "from_turn_id": branch_anchor, "head": target}),
            })
            .await
            .map_err(|err| AgentError::Session(err.to_string()))?;

        Ok(vec![ServerEvent::State {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            request_id: None,
            payload: json!({"state": "branch_head_checked_out", "head": target}),
        }])
    }

    async fn request_abort(&self) {
        let mut state = self.state.lock().await;
        *state = RunState::Aborting;
        let _ = self.abort_tx.send(()).await;
    }

    async fn is_aborting(&self) -> bool {
        let mut abort_rx = self.abort_rx.lock().await;
        abort_rx.try_recv().is_ok()
    }

    async fn handle_turn(&self, request: ClientRequest) -> Result<Vec<ServerEvent>> {
        let request_id = request.request_id().map(str::to_string);
        let message = request
            .message()
            .ok_or_else(|| AgentError::State("prompt request missing message".to_string()))?
            .to_string();

        {
            let mut store = self.config.session_store.lock().await;
            store
                .append(pi_protocol::session::SessionEntryKind::UserMessage {
                    text: message.clone(),
                })
                .await
                .map_err(|err| AgentError::Session(err.to_string()))?;
        }

        let mut events = Vec::new();
        events.push(ServerEvent::TurnStart {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            request_id: request_id.clone(),
            kind: "prompt".to_string(),
        });

        let completion = CompletionRequest {
            model: self.config.default_provider_model.clone(),
            messages: vec![CompletionMessage {
                role: "user".to_string(),
                content: message,
            }],
            tools: self
                .config
                .tool_registry
                .list()
                .iter()
                .map(|tool| tool.name.clone())
                .collect(),
            stream: true,
            temperature: 0.2,
            metadata: Default::default(),
        };
        let turn_started_at = Instant::now();
        let provider_name = self.config.provider.name().to_string();
        let model_name = completion.model.clone();
        let mut turn_usage = UsageTotals::default();
        // TODO: session_usage_totals scans all entries O(n) on every turn start.
        // Consider caching the running total in Agent state if session sizes grow.
        let mut session_usage = self.session_usage_totals().await?;

        let mut aggregate = String::new();
        let mut stream = self.config.provider.stream(completion, request_id.clone());
        let mut state = self.state.lock().await;
        *state = RunState::Running;
        drop(state);

        while let Some(event) = stream.next().await {
            match event {
                ProviderEvent::TextDelta {
                    text,
                    request_id: _,
                } => {
                    aggregate.push_str(&text);
                    events.push(ServerEvent::MessageUpdate {
                        v: protocol_version(),
                        id: Some(Uuid::new_v4().to_string()),
                        request_id: request_id.clone(),
                        delta: text,
                        done: false,
                    });
                }
                ProviderEvent::ThinkingDelta {
                    text,
                    request_id: _,
                } => {
                    aggregate.push_str(&text);
                    events.push(ServerEvent::MessageUpdate {
                        v: protocol_version(),
                        id: Some(Uuid::new_v4().to_string()),
                        request_id: request_id.clone(),
                        delta: text,
                        done: false,
                    });
                }
                ProviderEvent::ToolCall {
                    tool_name,
                    args,
                    request_id: _,
                } => {
                    let call_id = Uuid::new_v4().to_string();
                    events.push(ServerEvent::ToolCallStarted {
                        v: protocol_version(),
                        id: Some(Uuid::new_v4().to_string()),
                        request_id: request_id.clone(),
                        tool_name: tool_name.clone(),
                        call_id: call_id.clone(),
                        args: args.clone(),
                    });

                    match self
                        .execute_tool(tool_name, args, request_id.clone(), call_id.clone())
                        .await
                    {
                        Ok(output) => {
                            events.push(ServerEvent::ToolCallResult {
                                v: protocol_version(),
                                id: Some(Uuid::new_v4().to_string()),
                                request_id: request_id.clone(),
                                tool_name: output.tool_name,
                                call_id,
                                output: serde_json::to_value(&output.result)
                                    .unwrap_or_else(|_| json!({ "error": "serialize" })),
                            });
                        }
                        Err(err) => {
                            events.push(ServerEvent::ToolCallError {
                                v: protocol_version(),
                                id: Some(Uuid::new_v4().to_string()),
                                request_id: request_id.clone(),
                                tool_name: "unknown".to_string(),
                                call_id,
                                error: ProtocolErrorPayload::new(
                                    "tool_call_error",
                                    err.to_string(),
                                    None,
                                ),
                            });
                        }
                    }
                }
                ProviderEvent::Done { stop_reason, .. } => {
                    aggregate.push_str(&format!("\n[done:{stop_reason}]"));
                    break;
                }
                ProviderEvent::Usage {
                    input_tokens,
                    output_tokens,
                    cached_tokens,
                    cost_usd,
                    ..
                } => {
                    turn_usage.add_usage(input_tokens, output_tokens, cached_tokens, cost_usd);
                    session_usage.add_usage(input_tokens, output_tokens, cached_tokens, cost_usd);
                    self.append_usage_metadata(
                        &provider_name,
                        &model_name,
                        turn_started_at.elapsed().as_millis() as u64,
                        turn_usage,
                        session_usage,
                    )
                    .await?;
                }
                ProviderEvent::Error { message, .. } => {
                    events.push(make_error_event(
                        "provider_error",
                        message,
                        request_id.clone(),
                    ));
                    break;
                }
            }

            if self.is_aborting().await {
                events.push(ServerEvent::ToolCallError {
                    v: protocol_version(),
                    id: Some(Uuid::new_v4().to_string()),
                    request_id: request_id.clone(),
                    tool_name: "agent".to_string(),
                    call_id: Uuid::new_v4().to_string(),
                    error: ProtocolErrorPayload::new("aborted", "agent aborted", None),
                });
                break;
            }
        }

        if !aggregate.is_empty() {
            let mut store = self.config.session_store.lock().await;
            store
                .append(pi_protocol::session::SessionEntryKind::AssistantMessage {
                    text: aggregate,
                })
                .await
                .map_err(|err| AgentError::Session(err.to_string()))?;
        }

        events.push(ServerEvent::MessageUpdate {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            request_id: request_id.clone(),
            delta: String::new(),
            done: true,
        });
        events.push(ServerEvent::TurnEnd {
            v: protocol_version(),
            id: Some(Uuid::new_v4().to_string()),
            request_id,
            reason: Some("complete".to_string()),
        });

        {
            let mut state = self.state.lock().await;
            *state = RunState::Idle;
        }

        Ok(events)
    }

    async fn append_usage_metadata(
        &self,
        provider: &str,
        model: &str,
        elapsed_ms: u64,
        turn_usage: UsageTotals,
        session_usage: UsageTotals,
    ) -> Result<()> {
        let mut store = self.config.session_store.lock().await;
        store
            .append(pi_protocol::session::SessionEntryKind::SessionMetadata {
                payload: json!({
                    "type": "usage",
                    "provider": provider,
                    "model": model,
                    "timing": {
                        "turn_elapsed_ms": elapsed_ms,
                    },
                    "token": {
                        "turn": {
                            "input": turn_usage.input_tokens,
                            "output": turn_usage.output_tokens,
                            "cached": turn_usage.cached_tokens,
                        },
                        "session": {
                            "input": session_usage.input_tokens,
                            "output": session_usage.output_tokens,
                            "cached": session_usage.cached_tokens,
                        }
                    },
                    "cost": {
                        "turn_usd": turn_usage.cost_usd,
                        "session_usd": session_usage.cost_usd,
                    }
                }),
            })
            .await
            .map_err(|err| AgentError::Session(err.to_string()))?;
        Ok(())
    }

    async fn session_usage_totals(&self) -> Result<UsageTotals> {
        let store = self.config.session_store.lock().await;
        let mut totals = UsageTotals::default();
        for entry in &store.log.entries {
            let pi_protocol::session::SessionEntryKind::SessionMetadata { payload } = &entry.kind
            else {
                continue;
            };
            if payload.get("type").and_then(serde_json::Value::as_str) != Some("usage") {
                continue;
            }
            let session = &payload["token"]["session"];
            let input = session.get("input").and_then(serde_json::Value::as_u64);
            let output = session.get("output").and_then(serde_json::Value::as_u64);
            let cached = session.get("cached").and_then(serde_json::Value::as_u64);
            let cost = payload["cost"]
                .get("session_usd")
                .and_then(serde_json::Value::as_f64);
            if let (Some(input), Some(output), Some(cached), Some(cost)) =
                (input, output, cached, cost)
            {
                totals = UsageTotals {
                    input_tokens: input,
                    output_tokens: output,
                    cached_tokens: cached,
                    cost_usd: cost,
                };
            }
        }
        Ok(totals)
    }

    async fn execute_tool(
        &self,
        name: String,
        args: serde_json::Value,
        _request_id: Option<String>,
        call_id: String,
    ) -> Result<ToolCallResultOutput> {
        let tool_name = name.clone();
        let call = ToolCall {
            id: call_id.clone(),
            name,
            args,
        };

        let result = self
            .config
            .tool_registry
            .execute(
                &tool_name,
                &call,
                &self.config.tool_policy,
                &self.config.workspace_root,
            )
            .map_err(|err| AgentError::Tool(err.to_string()))?;

        let output_json =
            serde_json::to_value(&result).map_err(|err| AgentError::Tool(err.to_string()))?;

        {
            let mut store = self.config.session_store.lock().await;
            store
                .append(pi_protocol::session::SessionEntryKind::ToolResult {
                    tool_name: tool_name.clone(),
                    output: output_json,
                    success: result.status.as_str() == "ok",
                })
                .await
                .map_err(|err| AgentError::Session(err.to_string()))?;
        }

        Ok(ToolCallResultOutput { tool_name, result })
    }
}

fn resolve_branch_head(
    entries: &[pi_protocol::session::SessionEntry],
    children: &std::collections::HashMap<Uuid, Vec<Uuid>>,
    root: Uuid,
) -> Option<Uuid> {
    if !entries.iter().any(|entry| entry.entry_id == root) {
        return None;
    }

    let mut stack = vec![root];
    let mut reachable = std::collections::HashSet::new();
    while let Some(node) = stack.pop() {
        if !reachable.insert(node) {
            continue;
        }
        if let Some(next) = children.get(&node) {
            stack.extend(next.iter().copied());
        }
    }

    entries
        .iter()
        .rev()
        .find(|entry| reachable.contains(&entry.entry_id))
        .map(|entry| entry.entry_id)
}

struct ToolCallResultOutput {
    tool_name: String,
    result: ToolCallResult,
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream::{self, BoxStream};
    use pi_llm::{CompletionRequest, ModelCard};
    use pi_protocol::rpc::ClientRequest;

    #[derive(Debug)]
    struct UsageProvider;

    #[async_trait::async_trait]
    impl Provider for UsageProvider {
        fn name(&self) -> &'static str {
            "usage-mock"
        }

        async fn list_models(&self) -> pi_llm::Result<Vec<ModelCard>> {
            Ok(vec![ModelCard {
                id: "usage-model".to_string(),
                provider: self.name().to_string(),
                context_window: None,
            }])
        }

        fn stream(
            &self,
            _request: CompletionRequest,
            request_id: Option<String>,
        ) -> BoxStream<'static, ProviderEvent> {
            Box::pin(stream::iter(vec![
                ProviderEvent::TextDelta {
                    request_id: request_id.clone(),
                    text: "hello".to_string(),
                },
                ProviderEvent::Usage {
                    request_id: request_id.clone(),
                    input_tokens: 10,
                    output_tokens: 5,
                    cached_tokens: 2,
                    cost_usd: 0.01,
                },
                ProviderEvent::Usage {
                    request_id,
                    input_tokens: 1,
                    output_tokens: 4,
                    cached_tokens: 0,
                    cost_usd: 0.02,
                },
                ProviderEvent::Done {
                    request_id: None,
                    stop_reason: "complete".to_string(),
                },
            ]))
        }
    }

    fn temp_session_path() -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("pi-agent-usage-test-{}.jsonl", Uuid::new_v4()));
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
        path
    }

    #[tokio::test]
    async fn usage_events_append_session_metadata_with_turn_and_session_totals() {
        let session_path = temp_session_path();
        let session_store = SessionStore::new(&session_path)
            .await
            .expect("session store");

        let agent = Agent::new(AgentConfig {
            provider: Arc::new(UsageProvider),
            tool_registry: ToolRegistry::new(),
            tool_policy: pi_tools::Policy::safe_defaults(std::env::temp_dir()),
            session_store: Arc::new(Mutex::new(session_store)),
            workspace_root: std::env::temp_dir(),
            default_provider_model: "usage-model".to_string(),
            line_limit: 4000,
            extension_host: Arc::new(Mutex::new(RuntimeHost::default())),
        })
        .await;

        let request = ClientRequest::Prompt {
            v: protocol_version(),
            id: Some("req-1".to_string()),
            message: "count usage".to_string(),
            attachments: None,
        };

        let _ = agent.handle_request(request).await.expect("request");

        let store = agent.config.session_store.lock().await;
        let usage_entries: Vec<_> = store
            .log
            .entries
            .iter()
            .filter_map(|entry| {
                let pi_protocol::session::SessionEntryKind::SessionMetadata { payload } =
                    &entry.kind
                else {
                    return None;
                };
                (payload.get("type").and_then(serde_json::Value::as_str) == Some("usage"))
                    .then_some(payload)
            })
            .collect();

        assert_eq!(usage_entries.len(), 2);
        assert_eq!(usage_entries[0]["provider"], "usage-mock");
        assert_eq!(usage_entries[0]["model"], "usage-model");
        assert_eq!(usage_entries[0]["token"]["turn"]["input"], 10);
        assert_eq!(usage_entries[0]["token"]["session"]["input"], 10);
        assert_eq!(usage_entries[1]["token"]["turn"]["input"], 11);
        assert_eq!(usage_entries[1]["token"]["turn"]["output"], 9);
        assert_eq!(usage_entries[1]["token"]["session"]["input"], 11);
        assert_eq!(usage_entries[1]["token"]["session"]["cached"], 2);
        assert_eq!(usage_entries[1]["cost"]["turn_usd"], 0.03);
        assert_eq!(usage_entries[1]["cost"]["session_usd"], 0.03);

        let _ = std::fs::remove_file(&session_path);
    }
}
