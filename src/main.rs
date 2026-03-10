#![forbid(unsafe_code)]

use clap::{Parser, Subcommand, ValueEnum};
use pi_core::{run_rpc, Agent, AgentConfig};
use pi_llm::{MockProvider, Provider, ProviderError};
use pi_protocol::{parse_client_request, protocol_version, to_json_line, ServerEvent};
use pi_search::{SearchQuery, SearchService, SearchServiceConfig};
use pi_session::SessionStore;
use pi_tools::{default_registry, Policy};
use serde::Serialize;
use serde_json::json;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{self as tokio_io, AsyncBufReadExt};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "pi", about = "pi agent runtime")]
struct Cli {
    #[arg(long)]
    mode: Option<Mode>,

    #[arg(long, default_value = "mock")]
    provider: String,

    #[arg(long, default_value = "mock-tool-call")]
    model: String,

    #[arg(short = 'p', long)]
    prompt: Option<String>,

    #[arg()]
    positional_prompt: Option<String>,

    #[arg(long = "continue")]
    resume: bool,

    #[arg(long)]
    session: Option<PathBuf>,

    #[arg(long)]
    workspace: Option<PathBuf>,

    #[arg(long)]
    line_limit: Option<usize>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Protocol {
        #[command(subcommand)]
        command: ProtocolCommand,
    },
    Doctor,
    Search {
        query: String,
    },
    Info {
        package_or_extension: String,
    },
    #[command(name = "update-index")]
    UpdateIndex,
}

#[derive(Subcommand, Debug)]
enum ProtocolCommand {
    Schema {
        #[arg(short, long)]
        out: PathBuf,
    },
}

#[derive(Clone, ValueEnum, Debug)]
enum Mode {
    Interactive,
    Print,
    Rpc,
}

#[derive(Debug, thiserror::Error)]
enum CliError {
    #[error("session init failed")]
    SessionInit { message: String },
    #[error("search init failed")]
    SearchInit { message: String },
    #[error("provider operation failed")]
    Provider { message: String },
    #[error("unsupported operation")]
    Unsupported { command: String, reason: String },
    #[error("operation failed")]
    Operation { command: String, message: String },
}

#[derive(Debug, Serialize)]
struct CliErrorPayload {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

struct AppContext {
    agent: Agent,
    session_store: Arc<Mutex<SessionStore>>,
    search_service: Arc<SearchService>,
    provider: Arc<dyn Provider>,
    model: String,
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        print_structured_error(&err);
        std::process::exit(1);
    }
}

async fn run() -> Result<(), CliError> {
    let cli = Cli::parse();

    if let Some(Command::Protocol {
        command: ProtocolCommand::Schema { out },
    }) = cli.command
    {
        run_protocol_schema(out).await;
        return Ok(());
    }

    let workspace = cli
        .workspace
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    let line_limit = cli.line_limit.unwrap_or(1024 * 1024);

    let provider = build_provider(&cli.provider).await;

    let session_candidate = cli
        .session
        .clone()
        .unwrap_or_else(|| PathBuf::from(".pi/session.jsonl"));
    let session_path = SessionStore::resolve_session_path(session_candidate, workspace.clone())
        .map_err(|err| CliError::SessionInit {
            message: format!("failed to resolve session path: {err}"),
        })?;
    let mut session_store =
        SessionStore::new(session_path)
            .await
            .map_err(|err| CliError::SessionInit {
                message: format!("failed to open session store: {err}"),
            })?;

    if !cli.resume {
        session_store.reset().map_err(|err| CliError::Operation {
            command: "--continue".to_string(),
            message: format!("failed to reset session without --continue: {err}"),
        })?;
    }

    let search_service = SearchService::new(SearchServiceConfig {
        workspace_root: workspace.clone(),
        ..Default::default()
    })
    .await
    .map_err(|err| CliError::SearchInit {
        message: format!("failed to create search service: {err}"),
    })?;

    let session_store = Arc::new(Mutex::new(session_store));
    let policy = Policy::safe_defaults(workspace.clone());
    let registry = default_registry(search_service.clone());

    let config = AgentConfig {
        provider: provider.clone(),
        tool_registry: registry,
        session_store: session_store.clone(),
        tool_policy: policy,
        workspace_root: workspace.clone(),
        default_provider_model: cli.model.clone(),
        line_limit,
    };

    let agent = Agent::new(config).await;
    let app = AppContext {
        agent,
        session_store,
        search_service,
        provider,
        model: cli.model,
    };

    if let Some(command) = cli.command {
        run_command(command, &app).await?;
        return Ok(());
    }

    match cli.mode.unwrap_or(Mode::Interactive) {
        Mode::Rpc => {
            run_rpc(&app.agent)
                .await
                .map_err(|err| CliError::Operation {
                    command: "rpc".to_string(),
                    message: err.to_string(),
                })?;
        }
        Mode::Print => {
            let prompt = cli.prompt.as_deref().or(cli.positional_prompt.as_deref());
            if let Some(prompt) = prompt {
                let request_json = format!(
                    "{}",
                    serde_json::json!({
                        "v": protocol_version(),
                        "type": "prompt",
                        "id": Uuid::new_v4().to_string(),
                        "message": prompt,
                    })
                );

                match parse_client_request(&request_json) {
                    Ok(request) => match app.agent.handle_request(request).await {
                        Ok(events) => {
                            print_events_to_stdout(&events).await;
                        }
                        Err(err) => {
                            return Err(CliError::Operation {
                                command: "print".to_string(),
                                message: err.to_string(),
                            });
                        }
                    },
                    Err(err) => {
                        return Err(CliError::Operation {
                            command: "print".to_string(),
                            message: format!("request parse error: {err}"),
                        });
                    }
                }
            } else {
                return Err(CliError::Operation {
                    command: "print".to_string(),
                    message: "missing prompt (use [prompt] or -p/--prompt)".to_string(),
                });
            }
        }
        Mode::Interactive => run_interactive(app).await,
    }

    Ok(())
}

async fn run_command(command: Command, app: &AppContext) -> Result<(), CliError> {
    match command {
        Command::Doctor => {
            let session_summary = {
                let store = app.session_store.lock().await;
                store.to_text_summary()
            };

            let models = app
                .provider
                .list_models()
                .await
                .map_err(|err| CliError::Provider {
                    message: format_provider_error(err),
                })?;

            let report = json!({
                "status": "ok",
                "provider": app.provider.name(),
                "default_model": app.model,
                "models": models,
                "session": session_summary,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&report).unwrap_or_else(|_| report.to_string())
            );
            Ok(())
        }
        Command::Search { query } => {
            let response = app
                .search_service
                .search(SearchQuery {
                    text: query,
                    scope: None,
                    filters: Vec::new(),
                    limit: 50,
                    token: None,
                    offset: 0,
                })
                .await
                .map_err(|err| CliError::Operation {
                    command: "search".to_string(),
                    message: err.to_string(),
                })?;
            println!(
                "{}",
                serde_json::to_string_pretty(&response).unwrap_or_else(|_| "{}".to_string())
            );
            Ok(())
        }
        Command::Info {
            package_or_extension,
        } => Err(CliError::Unsupported {
            command: "info".to_string(),
            reason: format!(
                "metadata lookup is not implemented for '{package_or_extension}' in this build"
            ),
        }),
        Command::UpdateIndex => {
            app.search_service
                .rebuild_index()
                .await
                .map_err(|err| CliError::Operation {
                    command: "update-index".to_string(),
                    message: err.to_string(),
                })?;
            app.search_service
                .refresh_git_status()
                .await
                .map_err(|err| CliError::Operation {
                    command: "update-index".to_string(),
                    message: err.to_string(),
                })?;
            println!("{}", json!({"status": "ok", "command": "update-index"}));
            Ok(())
        }
        Command::Protocol { .. } => Ok(()),
    }
}

async fn run_protocol_schema(_out: PathBuf) {
    #[cfg(feature = "protocol-schema")]
    {
        if let Some(parent) = _out.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let schema = pi_protocol::schema_json();
        let text = serde_json::to_string_pretty(&schema).expect("schema");
        if let Err(err) = tokio::fs::write(&_out, text).await {
            let _ = writeln!(io::stderr(), "failed writing schema: {err}");
            return;
        }
        let _ = writeln!(io::stdout(), "wrote protocol schema to {}", _out.display());
    }

    #[cfg(not(feature = "protocol-schema"))]
    {
        let _ = tokio_io::AsyncWriteExt::write_all(
            &mut tokio_io::stdout(),
            b"{\"error\":\"protocol-schema feature is disabled\"}",
        )
        .await;
    }
}

async fn print_events_to_stdout(events: &[ServerEvent]) {
    for event in events {
        if let Ok(line) = to_json_line(event) {
            let _ = io::stdout().write_all(line.as_bytes());
        }
    }
}

async fn run_interactive(app: AppContext) {
    let mut out = io::stdout();
    let mut lines = tokio_io::BufReader::new(tokio_io::stdin()).lines();
    loop {
        let _ = out.write_all(b"> ");
        let _ = out.flush();

        let line = match lines.next_line().await {
            Ok(Some(value)) => value,
            Ok(None) => break,
            Err(_) => break,
        };

        if line.trim().is_empty() {
            continue;
        }

        if line.starts_with('/') {
            match handle_slash_command(line.trim(), &app).await {
                Ok(SlashResult::Continue) => continue,
                Ok(SlashResult::Exit) => break,
                Err(err) => {
                    let payload = cli_error_payload(&err);
                    let _ = out.write_all(
                        format!("{}\n", serde_json::to_string(&payload).unwrap_or_default())
                            .as_bytes(),
                    );
                    continue;
                }
            }
        }

        let request = match parse_client_request(
            &serde_json::json!({
                "v": protocol_version(),
                "type": "prompt",
                "id": Uuid::new_v4().to_string(),
                "message": line,
            })
            .to_string(),
        ) {
            Ok(value) => value,
            Err(err) => {
                let _ = out.write_all(format!("parse error: {err}\n").as_bytes());
                continue;
            }
        };

        match app.agent.handle_request(request).await {
            Ok(events) => {
                for event in events {
                    if let ServerEvent::MessageUpdate {
                        delta, done: false, ..
                    } = event
                    {
                        let _ = out.write_all(delta.as_bytes());
                    } else if let ServerEvent::MessageUpdate { done: true, .. } = event {
                        let _ = out.write_all(b"\n");
                    } else if let Ok(line) = to_json_line(&event) {
                        let _ = out.write_all(line.as_bytes());
                    }
                }
            }
            Err(err) => {
                let _ = out.write_all(format!("agent error: {err}\n").as_bytes());
            }
        }

        let _ = out.flush();
    }
}

enum SlashResult {
    Continue,
    Exit,
}

async fn handle_slash_command(
    command_line: &str,
    app: &AppContext,
) -> Result<SlashResult, CliError> {
    let mut parts = command_line.split_whitespace();
    let command = parts.next().unwrap_or_default();

    match command {
        "/help" => {
            println!("/help /model /tree /clear /compact /reload /exit");
            Ok(SlashResult::Continue)
        }
        "/model" => {
            let next = parts.next();
            if next.is_some() {
                return Err(CliError::Unsupported {
                    command: "/model".to_string(),
                    reason: "model switching is not implemented at runtime".to_string(),
                });
            }
            println!(
                "{}",
                json!({"provider": app.provider.name(), "model": app.model})
            );
            Ok(SlashResult::Continue)
        }
        "/tree" => {
            let summary = {
                let store = app.session_store.lock().await;
                store.to_text_summary()
            };
            println!("{}", json!({"session": summary}));
            Ok(SlashResult::Continue)
        }
        "/clear" => {
            let mut store = app.session_store.lock().await;
            store.reset().map_err(|err| CliError::Operation {
                command: "/clear".to_string(),
                message: err.to_string(),
            })?;
            println!("{}", json!({"status": "ok", "command": "/clear"}));
            Ok(SlashResult::Continue)
        }
        "/compact" => {
            let request = parse_client_request(
                &json!({
                    "v": protocol_version(),
                    "type": "compact",
                    "id": Uuid::new_v4().to_string(),
                })
                .to_string(),
            )
            .map_err(|err| CliError::Operation {
                command: "/compact".to_string(),
                message: err.to_string(),
            })?;
            let events =
                app.agent
                    .handle_request(request)
                    .await
                    .map_err(|err| CliError::Operation {
                        command: "/compact".to_string(),
                        message: err.to_string(),
                    })?;
            print_events_to_stdout(&events).await;
            Ok(SlashResult::Continue)
        }
        "/reload" => {
            app.search_service
                .rebuild_index()
                .await
                .map_err(|err| CliError::Operation {
                    command: "/reload".to_string(),
                    message: err.to_string(),
                })?;
            println!("{}", json!({"status": "ok", "command": "/reload"}));
            Ok(SlashResult::Continue)
        }
        "/exit" => Ok(SlashResult::Exit),
        _ => Err(CliError::Unsupported {
            command: command_line.to_string(),
            reason: "unknown slash command".to_string(),
        }),
    }
}

fn print_structured_error(err: &CliError) {
    let payload = cli_error_payload(err);
    let _ = writeln!(
        io::stderr(),
        "{}",
        serde_json::to_string(&payload).unwrap_or_else(|_| {
            "{\"code\":\"internal\",\"message\":\"failed to encode error\"}".to_string()
        })
    );
}

fn cli_error_payload(err: &CliError) -> CliErrorPayload {
    match err {
        CliError::SessionInit { message } => CliErrorPayload {
            code: "session_init_failed".to_string(),
            message: message.clone(),
            details: None,
        },
        CliError::SearchInit { message } => CliErrorPayload {
            code: "search_init_failed".to_string(),
            message: message.clone(),
            details: None,
        },
        CliError::Provider { message } => CliErrorPayload {
            code: "provider_failed".to_string(),
            message: message.clone(),
            details: None,
        },
        CliError::Unsupported { command, reason } => CliErrorPayload {
            code: "unsupported".to_string(),
            message: reason.clone(),
            details: Some(json!({"command": command})),
        },
        CliError::Operation { command, message } => CliErrorPayload {
            code: "operation_failed".to_string(),
            message: message.clone(),
            details: Some(json!({"command": command})),
        },
    }
}

fn format_provider_error(err: ProviderError) -> String {
    err.to_string()
}

async fn build_provider(kind: &str) -> Arc<dyn Provider> {
    match kind {
        "openai" => {
            #[cfg(feature = "openai")]
            {
                let base = std::env::var("PI_OPENAI_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:8000".to_string());
                let key = std::env::var("OPENAI_API_KEY").ok();
                return Arc::new(pi_llm::openai::OpenAIProvider::new(base, key));
            }
            #[cfg(not(feature = "openai"))]
            {
                Arc::new(MockProvider)
            }
        }
        _ => Arc::new(MockProvider),
    }
}
