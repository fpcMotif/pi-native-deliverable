#![forbid(unsafe_code)]

use pi_protocol::{
    make_error_event, parse_client_request, protocol_version, to_json_line, ServerEvent,
};
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt};

use crate::agent::Agent;

pub async fn run_rpc(agent: &Agent) -> std::io::Result<()> {
    let mut stdin = io::stdin();
    let mut stdout = io::stdout();

    let ready = ServerEvent::Ready {
        v: protocol_version(),
        id: Some("ready-event".to_string()),
        request_id: None,
        capabilities: serde_json::json!({
            "provider": agent.config.provider.name(),
            "models": [agent.config.default_provider_model.clone()],
            "tools": agent
                .config
                .tool_registry
                .list()
                .iter()
                .map(|tool| tool.name.clone())
                .collect::<Vec<_>>(),
            "line_limit": agent.config.line_limit,
            "session": {
                "tree": true,
                "compaction": true,
                "select_path": true,
                "fork": true,
                "checkout_branch_head": true,
            },
            "search": {
                "persistent": true,
                "scope_tokens": true,
            }
        }),
    };
    write_line(&mut stdout, ready).await?;

    let mut lines = tokio::io::BufReader::new(&mut stdin).lines();
    loop {
        let maybe_line = lines.next_line().await?;
        if maybe_line.is_none() {
            break;
        }

        let line = maybe_line.unwrap_or_default();
        if line.trim().is_empty() {
            continue;
        }
        if line.len() > agent.config.line_limit {
            write_line(
                &mut stdout,
                make_error_event(
                    "line_too_long",
                    "incoming line exceeds configured frame cap",
                    None,
                ),
            )
            .await?;
            continue;
        }

        let request = match parse_client_request(&line) {
            Ok(value) => value,
            Err(err) => {
                write_line(
                    &mut stdout,
                    make_error_event("invalid_request", err.to_string(), None),
                )
                .await?;
                continue;
            }
        };

        let events = match agent.handle_request(request).await {
            Ok(events) => events,
            Err(err) => vec![make_error_event("agent_error", err.to_string(), None)],
        };

        for event in events {
            if write_line(&mut stdout, event).await.is_err() {
                break;
            }
        }
    }

    Ok(())
}

async fn write_line(writer: &mut tokio::io::Stdout, event: ServerEvent) -> io::Result<()> {
    let line = to_json_line(&event).map_err(io::Error::other)?;
    writer.write_all(line.as_bytes()).await?;
    writer.flush().await
}
