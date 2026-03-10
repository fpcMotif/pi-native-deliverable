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
use tokio::io::{self as tokio_io, AsyncBufReadExt, AsyncWriteExt};
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
    std::process::exit(run().await);
}

/// Exit code for successful process completion.
const EXIT_SUCCESS: i32 = 0;
/// Exit code for invalid CLI/request input.
const EXIT_BAD_INPUT: i32 = 2;
/// Exit code for provider/tool/config/runtime failures.
const EXIT_RUNTIME_ERROR: i32 = 3;

async fn run() -> i32 {
    let cli = Cli::parse();

    if let Some(Command::Protocol {
        command: ProtocolCommand::Schema { out },
    }) = cli.command
    {
        run_protocol_schema(out).await;
        return EXIT_SUCCESS;
    }

    let workspace = cli
        .workspace
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    let line_limit = cli.line_limit.unwrap_or(1024 * 1024);

    let provider = build_provider(&cli.provider).await;

    let session_path =
        match SessionStore::resolve_session_path(".pi/session.jsonl", workspace.clone()) {
            Ok(path) => path,
            Err(err) => {
                let _ = writeln!(io::stderr(), "failed to resolve session path: {err}");
                return EXIT_RUNTIME_ERROR;
            }
        };
    let session_store = match SessionStore::new(session_path).await {
        Ok(store) => store,
        Err(err) => {
            let _ = writeln!(io::stderr(), "failed to open session store: {err}");
            return EXIT_RUNTIME_ERROR;
        }
    };

    let search_service = match SearchService::new(SearchServiceConfig {
        workspace_root: workspace.clone(),
        ..Default::default()
    })
    .await
    {
        Ok(service) => service,
        Err(err) => {
            let _ = writeln!(io::stderr(), "failed to create search service: {err}");
            return EXIT_RUNTIME_ERROR;
        }
    };

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
            if let Err(err) = run_rpc(&agent).await {
                let _ = writeln!(io::stderr(), "rpc runtime error: {err}");
                return EXIT_RUNTIME_ERROR;
            }
            EXIT_SUCCESS
        }
        Mode::Print => run_print_mode(&agent, cli.prompt.as_deref()).await,
        Mode::Interactive => {
            run_interactive(agent).await;
            EXIT_SUCCESS
        }
    }
}

async fn run_print_mode(agent: &Agent, prompt: Option<&str>) -> i32 {
    let prompt = match prompt {
        Some(value) => value,
        None => {
            let _ = writeln!(io::stderr(), "missing --prompt in print mode");
            return EXIT_BAD_INPUT;
        }
    };

    let request_json = serde_json::json!({
        "v": protocol_version(),
        "type": "prompt",
        "id": Uuid::new_v4().to_string(),
        "message": prompt,
    })
    .to_string();

    let request = match parse_client_request(&request_json) {
        Ok(request) => request,
        Err(err) => {
            let _ = writeln!(io::stderr(), "request parse error: {err}");
            return EXIT_BAD_INPUT;
        }
    };

    let events = match agent.handle_request(request).await {
        Ok(events) => events,
        Err(err) => {
            let _ = writeln!(io::stderr(), "error: {err}");
            return EXIT_RUNTIME_ERROR;
        }
    };

    let had_error = print_mode_emit(&events).await;
    if had_error {
        EXIT_RUNTIME_ERROR
    } else {
        EXIT_SUCCESS
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
        let mut stdout = tokio_io::stdout();
        let _ = stdout
            .write_all(b"{\"error\":\"protocol-schema feature is disabled\"}")
            .await;
    }
}

async fn print_mode_emit(events: &[ServerEvent]) -> bool {
    let mut had_error = false;
    for event in events {
        let line = match to_json_line(event) {
            Ok(line) => line,
            Err(err) => {
                had_error = true;
                let _ = writeln!(io::stderr(), "event serialization error: {err}");
                continue;
            }
        };

        if is_error_event(event) {
            had_error = true;
            let _ = io::stderr().write_all(line.as_bytes());
        } else {
            let _ = io::stdout().write_all(line.as_bytes());
        }
    }
    had_error
}

fn is_error_event(event: &ServerEvent) -> bool {
    matches!(
        event,
        ServerEvent::Error { .. } | ServerEvent::ToolCallError { .. }
    )
}

async fn run_interactive(agent: Agent) {
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
