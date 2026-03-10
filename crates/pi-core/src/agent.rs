#![forbid(unsafe_code)]

use futures::StreamExt;
use pi_ext::ExtensionRuntime;
use pi_llm::{CompletionMessage, CompletionRequest, Provider, ProviderEvent};
use pi_protocol::rpc::ClientRequest;
use pi_protocol::{make_error_event, protocol_version, ProtocolErrorPayload, ServerEvent};
use pi_session::SessionStore;
use pi_tools::{ToolCall, ToolCallResult, ToolRegistry};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
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
    pub extension_runtime: Option<Arc<Mutex<ExtensionRuntime>>>,
}

pub struct Agent {
    pub config: AgentConfig,
    state: Arc<Mutex<RunState>>,
    abort_tx: mpsc::Sender<()>,
    abort_rx: Arc<tokio::sync::Mutex<mpsc::Receiver<()>>>,
}

#[derive(Debug)]
pub struct CommandBus {
    sender: mpsc::Sender<Command>,
}

impl CommandBus {
    pub fn sender(&self) -> mpsc::Sender<Command> {
        self.sender.clone()
    }
}

#[derive(Debug)]
enum Command {
    Prompt(ClientRequest),
    Steer(ClientRequest),
    FollowUp(ClientRequest),
    Abort,
    GetState,
    Compact,
    NewSession,
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
            ClientRequest::Unknown { request_type, .. } => Ok(vec![make_error_event(
                "unsupported_message_type",
                format!("unsupported request type: {request_type}"),
                request_id,
            )]),
        };

        response
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
            tools: {
                let mut tools: Vec<String> = self
                    .config
                    .tool_registry
                    .list()
                    .iter()
                    .map(|tool| tool.name.clone())
                    .collect();
                if let Some(runtime) = &self.config.extension_runtime {
                    let ext_tools = runtime.lock().await.tool_names();
                    tools.extend(ext_tools);
                }
                tools
            },
            stream: true,
            temperature: 0.2,
            metadata: Default::default(),
        };

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
                ProviderEvent::Usage { .. } => {}
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

    async fn execute_tool(
        &self,
        name: String,
        args: serde_json::Value,
        _request_id: Option<String>,
        call_id: String,
    ) -> Result<ToolCallResultOutput> {
        let call = ToolCall {
            id: call_id.clone(),
            name: name.clone(),
            args,
        };

        let result = self
            .config
            .tool_registry
            .execute(&name, &call, &self.config.tool_policy, &self.config.workspace_root)
            .or_else(|err| {
                let is_extension_tool = if let Some(runtime) = &self.config.extension_runtime {
                    runtime
                        .try_lock()
                        .map(|rt| rt.tool_names().into_iter().any(|tool| tool == name))
                        .unwrap_or(false)
                } else {
                    false
                };

                if is_extension_tool {
                    Err(pi_tools::ToolError::Denied(
                        "extension tool execution is not yet available; hostcalls remain capability-gated".to_string(),
                    ))
                } else {
                    Err(err)
                }
            })
            .map_err(|err| AgentError::Tool(err.to_string()))?;

        let output_json =
            serde_json::to_value(&result).map_err(|err| AgentError::Tool(err.to_string()))?;

        {
            let mut store = self.config.session_store.lock().await;
            store
                .append(pi_protocol::session::SessionEntryKind::ToolResult {
                    tool_name: name.clone(),
                    output: output_json,
                    success: result.status.as_str() == "ok",
                })
                .await
                .map_err(|err| AgentError::Session(err.to_string()))?;
        }

        Ok(ToolCallResultOutput {
            tool_name: name,
            call_id,
            result,
        })
    }
}

struct ToolCallResultOutput {
    tool_name: String,
    call_id: String,
    result: ToolCallResult,
}
