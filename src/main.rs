#![forbid(unsafe_code)]

use clap::{Parser, Subcommand, ValueEnum};
use pi_core::{run_rpc, Agent, AgentConfig};
use pi_llm::{MockProvider, Provider};
use pi_protocol::{parse_client_request, protocol_version, to_json_line, ServerEvent};
use pi_search::{SearchService, SearchServiceConfig};
use pi_session::SessionStore;
use pi_tools::{default_registry, Policy};
use std::io::{self, Write};
use std::path::PathBuf;
use tokio::io::{self as tokio_io, AsyncBufReadExt};
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

    #[arg(long)]
    prompt: Option<String>,

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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Some(Command::Protocol {
        command: ProtocolCommand::Schema { out },
    }) = cli.command
    {
        run_protocol_schema(out).await;
        return;
    }

    let workspace = cli
        .workspace
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    let line_limit = cli.line_limit.unwrap_or(1024 * 1024);

    let provider = build_provider(&cli.provider).await;

    let session_path = SessionStore::resolve_session_path(".pi/session.jsonl", workspace.clone())
        .unwrap_or_else(|err| panic!("failed to resolve session path: {err}"));
    let session_store = SessionStore::new(session_path)
        .await
        .unwrap_or_else(|err| panic!("failed to open session store: {err}"));

    let search_service = SearchService::new(SearchServiceConfig {
        workspace_root: workspace.clone(),
        ..Default::default()
    })
    .await
    .unwrap_or_else(|err| panic!("failed to create search service: {err}"));

    let policy = Policy::safe_defaults(workspace.clone());
    let registry = default_registry(search_service.clone());

    let config = AgentConfig {
        provider,
        tool_registry: registry,
        session_store: std::sync::Arc::new(tokio::sync::Mutex::new(session_store)),
        tool_policy: policy,
        workspace_root: workspace.clone(),
        default_provider_model: cli.model,
        line_limit,
    };

    let agent = Agent::new(config).await;

    match cli.mode.unwrap_or(Mode::Interactive) {
        Mode::Rpc => {
            let _ = run_rpc(&agent).await;
        }
        Mode::Print => {
            if let Some(prompt) = cli.prompt.as_deref() {
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
                    Ok(request) => match agent.handle_request(request).await {
                        Ok(events) => {
                            print_events_to_stdout(&events).await;
                        }
                        Err(err) => {
                            let _ = writeln!(io::stdout(), "error: {err}");
                        }
                    },
                    Err(err) => {
                        let _ = writeln!(io::stdout(), "request parse error: {err}");
                    }
                }
            } else {
                let _ = writeln!(io::stdout(), "missing --prompt in print mode");
            }
        }
        Mode::Interactive => run_interactive(agent, search_service).await,
    }
}

async fn run_protocol_schema(out: PathBuf) {
    #[cfg(feature = "protocol-schema")]
    {
        if let Some(parent) = out.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let schema = pi_protocol::schema_json();
        let text = serde_json::to_string_pretty(&schema).expect("schema");
        if let Err(err) = tokio::fs::write(&out, text).await {
            let _ = writeln!(io::stderr(), "failed writing schema: {err}");
            return;
        }
        let _ = writeln!(io::stdout(), "wrote protocol schema to {}", out.display());
    }

    #[cfg(not(feature = "protocol-schema"))]
    {
        let _ = tokio_io::write_all(
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

async fn run_interactive(agent: Agent, search_service: std::sync::Arc<SearchService>) {
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

        if line == "/exit" {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }

        let line = search_service.complete_path_refs(&line, 40).await;

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

        match agent.handle_request(request).await {
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

async fn build_provider(kind: &str) -> std::sync::Arc<dyn Provider> {
    match kind {
        "openai" => {
            #[cfg(feature = "openai")]
            {
                let base = std::env::var("PI_OPENAI_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:8000".to_string());
                let key = std::env::var("OPENAI_API_KEY").ok();
                return std::sync::Arc::new(pi_llm::openai::OpenAIProvider::new(base, key));
            }
            #[cfg(not(feature = "openai"))]
            {
                std::sync::Arc::new(MockProvider)
            }
        }
        _ => std::sync::Arc::new(MockProvider),
    }
}
