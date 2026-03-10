#![forbid(unsafe_code)]

use clap::{Parser, Subcommand, ValueEnum};
use pi_core::{run_rpc, Agent, AgentConfig};
use pi_llm::{MockProvider, Provider};
use pi_protocol::{parse_client_request, protocol_version, to_json_line, ServerEvent};
use pi_search::SearchQuery;
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
    #[arg(value_name = "PROMPT")]
    prompt: Option<String>,

    #[arg(short = 'p', long = "print", value_name = "PROMPT")]
    print_prompt: Option<String>,

    #[arg(long)]
    mode: Option<Mode>,

    #[arg(long = "continue")]
    r#continue: bool,

    #[arg(long)]
    session: Option<PathBuf>,

    #[arg(long, default_value = "mock")]
    provider: String,

    #[arg(long, default_value = "mock-tool-call")]
    model: String,

    #[arg(long)]
    workspace: Option<PathBuf>,

    #[arg(long)]
    line_limit: Option<usize>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Doctor,
    Search {
        query: String,
    },
    Info {
        package_or_extension: String,
    },
    UpdateIndex,
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

    if let Some(prompt) = cli.print_prompt.as_deref() {
        run_print_once(prompt, &cli).await;
        return;
    }

    let workspace = cli
        .workspace
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    let line_limit = cli.line_limit.unwrap_or(1024 * 1024);

    let provider = build_provider(&cli.provider).await;

    let requested_session_path = cli
        .session
        .clone()
        .unwrap_or_else(|| PathBuf::from(".pi/session.jsonl"));
    let session_path =
        SessionStore::resolve_session_path(requested_session_path, workspace.clone())
            .unwrap_or_else(|err| panic!("failed to resolve session path: {err}"));
    let mut session_store = SessionStore::new(session_path)
        .await
        .unwrap_or_else(|err| panic!("failed to open session store: {err}"));

    if !cli.r#continue {
        session_store
            .reset()
            .unwrap_or_else(|err| panic!("failed to reset session store: {err}"));
    }

    let search_service = SearchService::new(SearchServiceConfig {
        workspace_root: workspace.clone(),
        ..Default::default()
    })
    .await
    .unwrap_or_else(|err| panic!("failed to create search service: {err}"));

    let policy = Policy::safe_defaults(workspace.clone());
    let registry = default_registry(search_service.clone());

    let model_name = cli.model.clone();
    let config = AgentConfig {
        provider,
        tool_registry: registry,
        session_store: std::sync::Arc::new(tokio::sync::Mutex::new(session_store)),
        tool_policy: policy,
        workspace_root: workspace.clone(),
        default_provider_model: model_name.clone(),
        line_limit,
    };

    let agent = Agent::new(config).await;

    match cli.command {
        Some(Command::Protocol {
            command: ProtocolCommand::Schema { out },
        }) => {
            run_protocol_schema(out).await;
            return;
        }
        Some(Command::Doctor) => {
            let _ = writeln!(io::stdout(), "doctor: ok");
            let _ = writeln!(io::stdout(), "workspace: {}", workspace.display());
            let _ = writeln!(io::stdout(), "provider: {}", cli.provider);
            return;
        }
        Some(Command::Search { query }) => {
            match search_service
                .search(SearchQuery {
                    text: query,
                    scope: None,
                    filters: Vec::new(),
                    limit: 10,
                    token: None,
                    offset: 0,
                })
                .await
            {
                Ok(result) => {
                    for item in result.items {
                        let _ = writeln!(io::stdout(), "{}", item.relative_path);
                    }
                }
                Err(err) => {
                    let _ = writeln!(io::stderr(), "search error: {err}");
                }
            }
            return;
        }
        Some(Command::Info {
            package_or_extension,
        }) => {
            let known = ["core", "search", "session", "tools", "llm", "ext"];
            let exists = known.iter().any(|name| name == &package_or_extension);
            if exists {
                let _ = writeln!(io::stdout(), "{package_or_extension}: available");
            } else {
                let _ = writeln!(io::stdout(), "{package_or_extension}: unknown");
            }
            return;
        }
        Some(Command::UpdateIndex) => {
            if let Err(err) = search_service.rebuild_index().await {
                let _ = writeln!(io::stderr(), "index update failed: {err}");
                return;
            }
            let _ = writeln!(io::stdout(), "index updated");
            return;
        }
        None => {}
    }

    if let Some(prompt) = cli.prompt.as_deref() {
        run_prompt_once(&agent, prompt).await;
        return;
    }

    match cli.mode.unwrap_or(Mode::Interactive) {
        Mode::Rpc => {
            let _ = run_rpc(&agent).await;
        }
        Mode::Print => {
            if let Some(prompt) = cli.prompt.as_deref() {
                run_prompt_once(&agent, prompt).await;
            } else {
                let _ = writeln!(io::stderr(), "missing prompt in print mode");
            }
        }
        Mode::Interactive => run_interactive(agent, &model_name).await,
    }
}

async fn run_print_once(prompt: &str, cli: &Cli) {
    let workspace = cli
        .workspace
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into()));
    let line_limit = cli.line_limit.unwrap_or(1024 * 1024);
    let provider = build_provider(&cli.provider).await;
    let requested_session_path = cli
        .session
        .clone()
        .unwrap_or_else(|| PathBuf::from(".pi/session.jsonl"));
    let session_path =
        SessionStore::resolve_session_path(requested_session_path, workspace.clone())
            .unwrap_or_else(|err| panic!("failed to resolve session path: {err}"));
    let mut session_store = SessionStore::new(session_path)
        .await
        .unwrap_or_else(|err| panic!("failed to open session store: {err}"));
    if !cli.r#continue {
        session_store
            .reset()
            .unwrap_or_else(|err| panic!("failed to reset session store: {err}"));
    }
    let search_service = SearchService::new(SearchServiceConfig {
        workspace_root: workspace.clone(),
        ..Default::default()
    })
    .await
    .unwrap_or_else(|err| panic!("failed to create search service: {err}"));
    let config = AgentConfig {
        provider,
        tool_registry: default_registry(search_service),
        session_store: std::sync::Arc::new(tokio::sync::Mutex::new(session_store)),
        tool_policy: Policy::safe_defaults(workspace.clone()),
        workspace_root: workspace,
        default_provider_model: cli.model.clone(),
        line_limit,
    };
    let agent = Agent::new(config).await;
    run_prompt_once(&agent, prompt).await;
}

async fn run_prompt_once(agent: &Agent, prompt: &str) {
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
            Ok(events) => print_print_response(&events),
            Err(err) => {
                let _ = writeln!(io::stderr(), "error: {err}");
            }
        },
        Err(err) => {
            let _ = writeln!(io::stderr(), "request parse error: {err}");
        }
    }
}

fn print_print_response(events: &[ServerEvent]) {
    let mut buffer = String::new();
    for event in events {
        if let ServerEvent::MessageUpdate { delta, .. } = event {
            buffer.push_str(delta);
        }
    }
    if buffer.is_empty() {
        for event in events {
            if let Ok(line) = to_json_line(event) {
                let _ = io::stdout().write_all(line.as_bytes());
            }
        }
    } else {
        let _ = writeln!(io::stdout(), "{buffer}");
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
        let _ = io::stdout().write_all(b"{\"error\":\"protocol-schema feature is disabled\"}");
    }
}

async fn run_interactive(agent: Agent, model: &str) {
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

        if handle_slash_command(&line, &agent, model, &mut out).await {
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

async fn handle_slash_command(
    line: &str,
    agent: &Agent,
    model: &str,
    out: &mut io::Stdout,
) -> bool {
    match line.trim() {
        "/help" => {
            let _ = out.write_all(b"/help /model /tree /clear /compact /exit /reload\n");
            false
        }
        "/model" => {
            let _ = out.write_all(format!("model: {model}\n").as_bytes());
            false
        }
        "/tree" => {
            let store = agent.config.session_store.lock().await;
            let (roots, children) = store.load_tree();
            let _ = out.write_all(
                format!(
                    "session tree: roots={} branches={} entries={}\n",
                    roots.len(),
                    children.len(),
                    store.log.entries.len()
                )
                .as_bytes(),
            );
            false
        }
        "/clear" => {
            let _ = out.write_all(b"\x1B[2J\x1B[1;1H");
            false
        }
        "/compact" => {
            let mut store = agent.config.session_store.lock().await;
            match store.compact(None).await {
                Ok(count) => {
                    let _ = out.write_all(format!("compacted {count} entries\n").as_bytes());
                }
                Err(err) => {
                    let _ = out.write_all(format!("compact error: {err}\n").as_bytes());
                }
            }
            false
        }
        "/reload" => {
            let _ = out.write_all(b"reload complete\n");
            false
        }
        "/exit" => true,
        _ => false,
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
