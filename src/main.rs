#![forbid(unsafe_code)]

use clap::{Parser, Subcommand, ValueEnum};
use pi_core::agent::AgentError;
use pi_core::{run_rpc, Agent, AgentConfig};
use pi_llm::{MockProvider, Provider};
use pi_protocol::{parse_client_request, protocol_version, to_json_line, ServerEvent};
use pi_search::{SearchService, SearchServiceConfig};
use pi_session::SessionStore;
use pi_tools::{default_registry, Policy};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process;
use tokio::io::{self as tokio_io, AsyncBufReadExt, AsyncWriteExt};
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(name = "pi", about = "pi agent runtime")]
struct Cli {
    #[arg(short = 'p', long = "print")]
    print: bool,

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
    let exit_code = run().await;
    process::exit(exit_code);
}

#[repr(i32)]
enum ExitCode {
    Success = 0,
    InvalidInput = 2,
    Runtime = 10,
    ProviderFailure = 20,
    ToolFailure = 21,
}

async fn run() -> i32 {
    let cli = Cli::parse();
    let mode = if cli.print {
        Mode::Print
    } else {
        cli.mode.clone().unwrap_or(Mode::Interactive)
    };

    if let Some(Command::Protocol {
        command: ProtocolCommand::Schema { out },
    }) = cli.command
    {
        run_protocol_schema(out).await;
        return ExitCode::Success as i32;
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

    match mode {
        Mode::Rpc => {
            let _ = run_rpc(&agent).await;
            ExitCode::Success as i32
        }
        Mode::Print => run_print_mode(&agent, cli.prompt.as_deref()).await as i32,
        Mode::Interactive => {
            run_interactive(agent).await;
            ExitCode::Success as i32
        }
    }
}

async fn run_print_mode(agent: &Agent, prompt: Option<&str>) -> ExitCode {
    let Some(prompt) = prompt else {
        let _ = writeln!(io::stderr(), "missing --prompt in print mode");
        return ExitCode::InvalidInput;
    };

    let request_json = format!(
        "{}",
        serde_json::json!({
            "v": protocol_version(),
            "type": "prompt",
            "id": Uuid::new_v4().to_string(),
            "message": prompt,
        })
    );

    let request = match parse_client_request(&request_json) {
        Ok(request) => request,
        Err(err) => {
            let _ = writeln!(io::stderr(), "request parse error: {err}");
            return ExitCode::InvalidInput;
        }
    };

    let events = match agent.handle_request(request).await {
        Ok(events) => events,
        Err(err) => {
            emit_runtime_error(&err);
            return ExitCode::Runtime;
        }
    };

    let mut assistant_output = String::new();
    let mut saw_provider_error = false;
    let mut saw_tool_error = false;

    for event in events {
        match event {
            ServerEvent::MessageUpdate {
                delta, done: false, ..
            } => assistant_output.push_str(&delta),
            ServerEvent::Error { error, .. } if error.code == "provider_error" => {
                saw_provider_error = true;
                let _ = writeln!(io::stderr(), "provider error: {}", error.message);
            }
            ServerEvent::ToolCallError { error, .. } => {
                saw_tool_error = true;
                let _ = writeln!(io::stderr(), "tool error: {}", error.message);
            }
            _ => {}
        }
    }

    if saw_provider_error {
        return ExitCode::ProviderFailure;
    }
    if saw_tool_error {
        return ExitCode::ToolFailure;
    }

    let _ = io::stdout().write_all(assistant_output.as_bytes());
    ExitCode::Success
}

fn emit_runtime_error(err: &AgentError) {
    let _ = writeln!(io::stderr(), "runtime error: {err}");
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
        let mut out = tokio_io::stdout();
        let _ = out
            .write_all(b"{\"error\":\"protocol-schema feature is disabled\"}")
            .await;
    }
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
