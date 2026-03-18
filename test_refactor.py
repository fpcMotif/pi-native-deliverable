import re

with open("src/main.rs", "r") as f:
    content = f.read()

agent_creation = """    let line_limit = cli.line_limit.unwrap_or(1024 * 1024);
    let mut catalog = Catalog::discover(&workspace);
    print_catalog_diagnostics(&catalog);

    let provider = build_provider(&cli.provider).await;

    let requested_session_path = cli
        .session
        .clone()
        .unwrap_or_else(|| PathBuf::from(".pi/session.jsonl"));
    let session_path =
        SessionStore::resolve_session_path(requested_session_path, workspace.clone())?;
    let mut session_store = SessionStore::new(session_path).await?;

    if !cli.r#continue {
        session_store
            .reset()
            .unwrap_or_else(|err| panic!("failed to reset session store: {err}"));
    }

    let search_service = SearchService::new(SearchServiceConfig {
        workspace_root: workspace.clone(),
        ..Default::default()
    })
    .await?;

    let policy = Policy::safe_defaults(workspace.clone());
    let registry = default_registry(search_service.clone());

    let mut extension_host = RuntimeHost::default();
    let extensions_dir = cli
        .extensions_dir
        .clone()
        .unwrap_or_else(|| workspace.join(".pi/extensions"));
    if let Ok(entries) = std::fs::read_dir(&extensions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                let _ = extension_host.load_extension_manifest(path);
            }
        }
    }

    let model_name = cli.model.clone();
    let config = AgentConfig {
        provider,
        tool_registry: registry,
        session_store: std::sync::Arc::new(tokio::sync::Mutex::new(session_store)),
        tool_policy: policy,
        workspace_root: workspace.clone(),
        default_provider_model: model_name.clone(),
        line_limit,
        extension_host: std::sync::Arc::new(tokio::sync::Mutex::new(extension_host)),
    };

    let agent = Agent::new(config).await;"""

routing_logic = """    match cli.command {
        Some(Command::Protocol {
            command: ProtocolCommand::Schema { out },
        }) => {
            run_protocol_schema(out).await;
            return Ok(());
        }
        Some(Command::Doctor) => {
            let _ = writeln!(io::stdout(), "doctor: ok");
            let _ = writeln!(io::stdout(), "workspace: {}", workspace.display());
            let _ = writeln!(io::stdout(), "provider: {}", cli.provider);
            return Ok(());
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
            return Ok(());
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
            return Ok(());
        }
        Some(Command::UpdateIndex) => {
            if let Err(err) = search_service.rebuild_index().await {
                let _ = writeln!(io::stderr(), "index update failed: {err}");
                return Ok(());
            }
            let _ = writeln!(io::stdout(), "index updated");
            return Ok(());
        }
        None => {}
    }"""

if agent_creation in content and routing_logic in content:
    new_main = """    let workspace = match cli.workspace {
        Some(path) => path,
        None => std::env::current_dir()?,
    };

    let (agent, mut catalog, search_service) = create_agent(&cli, workspace.clone()).await?;

    if handle_command(&cli, &workspace, search_service.clone()).await? {
        return Ok(());
    }"""

    new_content = content.replace(agent_creation + "\n\n" + routing_logic, new_main)

    new_functions = """
async fn create_agent(
    cli: &Cli,
    workspace: PathBuf,
) -> Result<(Agent, Catalog, std::sync::Arc<SearchService>), Box<dyn std::error::Error>> {
    let line_limit = cli.line_limit.unwrap_or(1024 * 1024);
    let catalog = Catalog::discover(&workspace);
    print_catalog_diagnostics(&catalog);

    let provider = build_provider(&cli.provider).await;

    let requested_session_path = cli
        .session
        .clone()
        .unwrap_or_else(|| PathBuf::from(".pi/session.jsonl"));
    let session_path =
        SessionStore::resolve_session_path(requested_session_path, workspace.clone())?;
    let mut session_store = SessionStore::new(session_path).await?;

    if !cli.r#continue {
        session_store
            .reset()
            .unwrap_or_else(|err| panic!("failed to reset session store: {err}"));
    }

    let search_service = SearchService::new(SearchServiceConfig {
        workspace_root: workspace.clone(),
        ..Default::default()
    })
    .await?;

    let policy = Policy::safe_defaults(workspace.clone());
    let registry = default_registry(search_service.clone());

    let mut extension_host = RuntimeHost::default();
    let extensions_dir = cli
        .extensions_dir
        .clone()
        .unwrap_or_else(|| workspace.join(".pi/extensions"));
    if let Ok(entries) = std::fs::read_dir(&extensions_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                let _ = extension_host.load_extension_manifest(path);
            }
        }
    }

    let model_name = cli.model.clone();
    let config = AgentConfig {
        provider,
        tool_registry: registry,
        session_store: std::sync::Arc::new(tokio::sync::Mutex::new(session_store)),
        tool_policy: policy,
        workspace_root: workspace.clone(),
        default_provider_model: model_name.clone(),
        line_limit,
        extension_host: std::sync::Arc::new(tokio::sync::Mutex::new(extension_host)),
    };

    let agent = Agent::new(config).await;
    Ok((agent, catalog, search_service))
}

async fn handle_command(
    cli: &Cli,
    workspace: &std::path::Path,
    search_service: std::sync::Arc<SearchService>,
) -> Result<bool, Box<dyn std::error::Error>> {
    match &cli.command {
        Some(Command::Protocol {
            command: ProtocolCommand::Schema { out },
        }) => {
            run_protocol_schema(out.clone()).await;
            return Ok(true);
        }
        Some(Command::Doctor) => {
            let _ = writeln!(io::stdout(), "doctor: ok");
            let _ = writeln!(io::stdout(), "workspace: {}", workspace.display());
            let _ = writeln!(io::stdout(), "provider: {}", cli.provider);
            return Ok(true);
        }
        Some(Command::Search { query }) => {
            match search_service
                .search(SearchQuery {
                    text: query.clone(),
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
            return Ok(true);
        }
        Some(Command::Info {
            package_or_extension,
        }) => {
            let known = ["core", "search", "session", "tools", "llm", "ext"];
            let exists = known.iter().any(|name| name == package_or_extension);
            if exists {
                let _ = writeln!(io::stdout(), "{package_or_extension}: available");
            } else {
                let _ = writeln!(io::stdout(), "{package_or_extension}: unknown");
            }
            return Ok(true);
        }
        Some(Command::UpdateIndex) => {
            if let Err(err) = search_service.rebuild_index().await {
                let _ = writeln!(io::stderr(), "index update failed: {err}");
                return Ok(true);
            }
            let _ = writeln!(io::stdout(), "index updated");
            return Ok(true);
        }
        None => Ok(false)
    }
}
"""

    # insert before run_prompt_once
    run_prompt_once_idx = new_content.find("async fn run_prompt_once(")
    new_content = new_content[:run_prompt_once_idx] + new_functions + "\n" + new_content[run_prompt_once_idx:]

    with open("src/main.rs", "w") as f:
        f.write(new_content)
    print("Done refactoring")
else:
    print("Could not find the target code blocks")
