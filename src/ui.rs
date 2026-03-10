#![forbid(unsafe_code)]

use pi_core::Agent;
use pi_protocol::rpc::ClientRequest;
use pi_protocol::{protocol_version, to_json_line, ServerEvent};
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{self as tokio_io, AsyncBufReadExt};
use tokio::sync::mpsc;
use uuid::Uuid;

const COMMANDS: [&str; 7] = [
    "/help", "/model", "/tree", "/clear", "/compact", "/exit", "/reload",
];

pub async fn run_interactive_ui(agent: Arc<Agent>) {
    let mut app = UiApp::new(agent);
    if let Err(err) = app.run().await {
        let _ = writeln!(io::stderr(), "interactive ui error: {err}");
    }
}

struct UiApp {
    agent: Arc<Agent>,
    transcript: Vec<String>,
    tool_panel: Vec<String>,
    assistant_line: String,
    prompt: String,
}

impl UiApp {
    fn new(agent: Arc<Agent>) -> Self {
        Self {
            agent,
            transcript: vec!["TUI ready. Type /help for slash commands.".to_string()],
            tool_panel: Vec::new(),
            assistant_line: String::new(),
            prompt: "> ".to_string(),
        }
    }

    async fn run(&mut self) -> io::Result<()> {
        let (input_tx, mut input_rx) = mpsc::channel::<String>(32);
        tokio::spawn(async move {
            let mut lines = tokio_io::BufReader::new(tokio_io::stdin()).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = input_tx.send(line).await;
            }
        });

        let (event_tx, mut event_rx) = mpsc::channel::<ServerEvent>(256);

        let mut should_exit = false;
        while !should_exit {
            self.render()?;
            tokio::select! {
                maybe_line = input_rx.recv() => {
                    let Some(line) = maybe_line else { break; };
                    if self.on_line(line, event_tx.clone()).await {
                        should_exit = true;
                    }
                }
                maybe_event = event_rx.recv() => {
                    if let Some(event) = maybe_event {
                        self.on_event(event);
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(120)) => {}
            }
        }

        Ok(())
    }

    async fn on_line(&mut self, line: String, event_tx: mpsc::Sender<ServerEvent>) -> bool {
        let input = line.trim().to_string();
        if input.is_empty() {
            return false;
        }

        self.transcript.push(format!("> {input}"));

        if input.starts_with('/') {
            return self.handle_slash_command(input).await;
        }

        self.assistant_line.clear();
        let agent = self.agent.clone();
        tokio::spawn(async move {
            let request = ClientRequest::Prompt {
                v: protocol_version().to_string(),
                id: Some(Uuid::new_v4().to_string()),
                message: input,
                attachments: None,
            };
            match agent.handle_request(request).await {
                Ok(events) => {
                    for event in events {
                        let _ = event_tx.send(event).await;
                    }
                }
                Err(err) => {
                    let _ = event_tx
                        .send(ServerEvent::Error {
                            v: protocol_version().to_string(),
                            id: Some(Uuid::new_v4().to_string()),
                            request_id: None,
                            error: pi_protocol::ProtocolErrorPayload::new(
                                "agent_error",
                                err.to_string(),
                                None,
                            ),
                        })
                        .await;
                }
            }
        });

        false
    }

    async fn handle_slash_command(&mut self, input: String) -> bool {
        let mut parts = input.split_whitespace();
        let raw = parts.next().unwrap_or_default();
        let command = self.complete_command(raw);

        match command.as_str() {
            "/help" => self.transcript.push(
                "slash commands: /help /model [name] /tree /clear /compact /exit /reload"
                    .to_string(),
            ),
            "/model" => {
                if let Some(model) = parts.next() {
                    self.agent.set_model(model.to_string()).await;
                    self.transcript.push(format!("model set: {model}"));
                } else {
                    let model = self.agent.current_model().await;
                    self.transcript.push(format!("model: {model}"));
                }
            }
            "/tree" => {
                let payload = self.agent.tree_payload().await;
                self.transcript.push(format!("tree: {payload}"));
            }
            "/clear" => {
                self.transcript.clear();
                self.tool_panel.clear();
            }
            "/compact" => {
                let request = ClientRequest::Compact {
                    v: protocol_version().to_string(),
                    id: Some(Uuid::new_v4().to_string()),
                    reserve_tokens: None,
                    keep_recent_tokens: None,
                };
                match self.agent.handle_request(request).await {
                    Ok(events) => self
                        .transcript
                        .push(format!("compact: {} events", events.len())),
                    Err(err) => self.transcript.push(format!("compact error: {err}")),
                }
            }
            "/reload" => match self.agent.reload_session().await {
                Ok(payload) => self.transcript.push(format!("reload: {payload}")),
                Err(err) => self.transcript.push(format!("reload error: {err}")),
            },
            "/exit" => return true,
            other => {
                self.transcript.push(format!("unknown command: {other}"));
                let suggestions = COMMANDS
                    .iter()
                    .copied()
                    .filter(|cmd| cmd.starts_with(raw))
                    .collect::<Vec<_>>()
                    .join(" ");
                if !suggestions.is_empty() {
                    self.transcript.push(format!("completions: {suggestions}"));
                }
            }
        }
        false
    }

    fn complete_command(&mut self, raw: &str) -> String {
        if COMMANDS.contains(&raw) {
            return raw.to_string();
        }
        let matches: Vec<&str> = COMMANDS
            .iter()
            .copied()
            .filter(|candidate| candidate.starts_with(raw))
            .collect();
        if matches.len() == 1 {
            let completed = matches[0].to_string();
            self.transcript
                .push(format!("autocompleted {raw} -> {completed}"));
            completed
        } else {
            raw.to_string()
        }
    }

    fn on_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::MessageUpdate {
                delta, done: false, ..
            } => self.assistant_line.push_str(&delta),
            ServerEvent::MessageUpdate { done: true, .. } => {
                if !self.assistant_line.is_empty() {
                    self.transcript
                        .push(format!("assistant: {}", self.assistant_line));
                    self.assistant_line.clear();
                }
            }
            ServerEvent::ToolCallStarted {
                tool_name, args, ..
            } => {
                self.tool_panel.push(format!("start {tool_name}: {args}"));
            }
            ServerEvent::ToolCallResult {
                tool_name, output, ..
            } => {
                self.tool_panel
                    .push(format!("result {tool_name}: {output}"));
            }
            ServerEvent::ToolCallError {
                tool_name, error, ..
            } => {
                self.tool_panel
                    .push(format!("error {tool_name}: {}", error.message));
            }
            other => {
                if let Ok(line) = to_json_line(&other) {
                    self.transcript.push(line.trim().to_string());
                }
            }
        }
    }

    fn render(&self) -> io::Result<()> {
        let mut out = io::stdout();
        write!(out, "\x1B[2J\x1B[H")?;
        writeln!(out, "pi interactive TUI")?;
        writeln!(out, "{}", "─".repeat(60))?;
        writeln!(out, "Transcript")?;
        for line in self.transcript.iter().rev().take(8).rev() {
            writeln!(out, "{line}")?;
        }
        if !self.assistant_line.is_empty() {
            writeln!(out, "assistant(stream): {}", self.assistant_line)?;
        }
        writeln!(out, "{}", "─".repeat(60))?;
        writeln!(out, "Tool panel")?;
        for line in self.tool_panel.iter().rev().take(5).rev() {
            writeln!(out, "{line}")?;
        }
        writeln!(out, "{}", "─".repeat(60))?;
        writeln!(out, "{}", self.prompt)?;
        out.flush()
    }
}
