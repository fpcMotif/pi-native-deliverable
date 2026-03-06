#![forbid(unsafe_code)]

use pi_search::{GrepMode, SearchQuery, SearchService};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

pub type Result<T> = std::result::Result<T, ToolError>;

pub const DEFAULT_WRITE_LIMIT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolStatus {
    Ok,
    Denied,
    Error,
}

impl ToolStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Denied => "denied",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallResult {
    pub stdout: String,
    pub status: ToolStatus,
    pub error: Option<String>,
    pub truncated: bool,
    pub metadata: BTreeMap<String, Value>,
}

#[derive(Debug, Clone)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub schema: Value,
}

pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> Value;
    fn execute(&self, call: &ToolCall, policy: &Policy, cwd: &Path) -> Result<ToolCallResult>;
}

pub struct ToolRegistry {
    tools: std::collections::HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: std::collections::HashMap::new(),
        }
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        self.tools.insert(tool.name().to_string(), Box::new(tool));
    }

    pub fn execute(&self, name: &str, call: &ToolCall, policy: &Policy, cwd: &Path) -> Result<ToolCallResult> {
        let tool = self.tools.get(name).ok_or_else(|| ToolError::not_found(name))?;
        tool.execute(call, policy, cwd)
    }

    pub fn list(&self) -> Vec<ToolDefinition> {
        self.tools
            .values()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                schema: tool.schema(),
            })
            .collect()
    }

    pub fn schema_json(&self) -> Value {
        let tools: Vec<_> = self
            .list()
            .into_iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "schema": tool.schema,
                })
            })
            .collect();

        json!({"tools": tools})
    }
}

#[derive(Debug, Clone)]
pub struct Policy {
    pub workspace_root: PathBuf,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub command_timeout_ms: u64,
    pub deny_write_paths: Vec<String>,
    pub max_file_size: usize,
}

impl Policy {
    pub fn safe_defaults(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
            max_stdout_bytes: 32 * 1024,
            max_stderr_bytes: 8 * 1024,
            command_timeout_ms: 5_000,
            deny_write_paths: vec![
                ".env".to_string(),
                ".env.local".to_string(),
                ".bash_history".to_string(),
                "id_rsa".to_string(),
                "id_rsa.pub".to_string(),
            ],
            max_file_size: DEFAULT_WRITE_LIMIT_BYTES,
        }
    }

    pub fn canonicalize_path(&self, path: &str, cwd: &Path) -> Result<PathBuf> {
        let requested = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            cwd.join(path)
        };

        let workspace_root = self
            .workspace_root
            .canonicalize()
            .map_err(|_| ToolError::invalid("workspace", "invalid workspace root"))?;

        if requested.exists() {
            let normalized = requested
                .canonicalize()
                .map_err(|_| ToolError::invalid("path", "path does not exist"))?;
            if !normalized.starts_with(&workspace_root) {
                return Err(ToolError::denied(format!(
                    "path escapes workspace: {}",
                    normalized.display()
                )));
            }
            return Ok(normalized);
        }

        let parent = requested.parent().unwrap_or(Path::new("."));
        if !parent.exists() {
            return Err(ToolError::invalid("path", "parent directory does not exist"));
        }
        let parent = parent
            .canonicalize()
            .map_err(|_| ToolError::invalid("path", "invalid parent path"))?;
        if !parent.starts_with(&workspace_root) {
            return Err(ToolError::denied(format!(
                "path escapes workspace: {}",
                parent.display()
            )));
        }

        let file_name = requested
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| ToolError::invalid("path", "invalid file name"))?;
        Ok(parent.join(file_name))
    }

    pub fn can_write_path(&self, path: &Path) -> bool {
        path.components().any(|component| {
            if let Component::Normal(component) = component {
                let value = OsStr::to_string_lossy(component);
                self.deny_write_paths.iter().any(|deny| deny == value.as_ref())
            } else {
                false
            }
        })
    }
}

#[derive(Debug)]
pub struct ReadTool;

#[derive(Debug)]
pub struct WriteTool;

#[derive(Debug)]
pub struct EditTool;

#[derive(Debug)]
pub struct BashTool;

#[derive(Debug)]
pub struct GrepTool {
    search_service: std::sync::Arc<SearchService>,
}

#[derive(Debug)]
pub struct FindTool {
    search_service: std::sync::Arc<SearchService>,
}

#[derive(Debug)]
pub struct LsTool;

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("tool denied: {0}")]
    Denied(String),
    #[error("tool error: {0}")]
    Error(String),
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error("invalid argument: {0}")]
    InvalidArg(String),
    #[error("io: {0}")]
    Io(#[from] io::Error),
}

impl ToolError {
    fn denied(message: impl Into<String>) -> Self {
        Self::Denied(message.into())
    }

    fn invalid(field: &str, message: impl Into<String>) -> Self {
        Self::InvalidArg(format!("{field}: {}", message.into()))
    }

    fn not_found(name: impl Into<String>) -> Self {
        Self::NotFound(name.into())
    }
}

fn take_max(value: String, max: usize) -> (String, bool) {
    if value.len() > max {
        let mut value = value;
        value.truncate(max);
        (value, true)
    } else {
        (value, false)
    }
}

fn read_arg<T>(call: &ToolCall, name: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    call.args
        .get(name)
        .ok_or_else(|| ToolError::invalid(name, "missing"))
        .and_then(|value| serde_json::from_value(value.clone()).map_err(|err| ToolError::invalid(name, err.to_string())))
}

impl Tool for ReadTool {
    fn name(&self) -> &'static str {
        "read"
    }

    fn description(&self) -> &'static str {
        "Read text file safely with binary detection and truncation."
    }

    fn schema(&self) -> Value {
        json!({
            "name": "read",
            "description": "Read file",
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "max_bytes": {"type": "integer", "minimum": 1},
            },
            "required": ["path"]
        })
    }

    fn execute(&self, call: &ToolCall, policy: &Policy, cwd: &Path) -> Result<ToolCallResult> {
        let path = read_arg::<String>(call, "path")?;
        let max = call.args.get("max_bytes")
            .and_then(Value::as_u64)
            .unwrap_or(policy.max_file_size as u64) as usize;

        let normalized = policy.canonicalize_path(&path, cwd)?;
        let mut bytes = Vec::new();
        let mut file = fs::File::open(&normalized)?;
        file.read_to_end(&mut bytes)?;
        if bytes.iter().any(|byte| *byte == 0) {
            return Err(ToolError::denied("refusing to read binary file"));
        }

        let (stdout, truncated) = take_max(String::from_utf8_lossy(&bytes).to_string(), max);
        Ok(ToolCallResult {
            stdout,
            status: ToolStatus::Ok,
            error: None,
            truncated,
            metadata: BTreeMap::from_iter([
                ("path".to_string(), json!(normalized.to_string_lossy().to_string())),
                ("bytes".to_string(), json!(bytes.len() as u64)),
            ]),
        })
    }
}

impl Tool for WriteTool {
    fn name(&self) -> &'static str {
        "write"
    }

    fn description(&self) -> &'static str {
        "Create or overwrite file content with explicit allow-list restrictions."
    }

    fn schema(&self) -> Value {
        json!({
            "name": "write",
            "description": "Write file content",
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "content": {"type": "string"},
                "append": {"type": "boolean"}
            },
            "required": ["path", "content"]
        })
    }

    fn execute(&self, call: &ToolCall, policy: &Policy, cwd: &Path) -> Result<ToolCallResult> {
        let path = read_arg::<String>(call, "path")?;
        let content = read_arg::<String>(call, "content")?;
        let append = call.args.get("append").and_then(Value::as_bool).unwrap_or(false);

        if content.len() > policy.max_file_size {
            return Err(ToolError::Error(format!(
                "content exceeds max size {}",
                policy.max_file_size
            )));
        }

        let normalized = policy.canonicalize_path(&path, cwd)?;
        if policy.can_write_path(&normalized) {
            return Err(ToolError::denied(format!("writing denied: {}", normalized.display())));
        }

        if let Some(parent) = normalized.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut options = OpenOptions::new();
        options.create(true).write(true);
        if append {
            options.append(true);
        } else {
            options.truncate(true);
        }

        let mut file = options.open(&normalized)?;
        file.write_all(content.as_bytes())?;

        Ok(ToolCallResult {
            stdout: "ok".to_string(),
            status: ToolStatus::Ok,
            error: None,
            truncated: false,
            metadata: BTreeMap::from_iter([(
                "path".to_string(),
                json!(normalized.to_string_lossy().to_string()),
            )]),
        })
    }
}

impl Tool for EditTool {
    fn name(&self) -> &'static str {
        "edit"
    }

    fn description(&self) -> &'static str {
        "Replace text occurrences in file content."
    }

    fn schema(&self) -> Value {
        json!({
            "name": "edit",
            "description": "Edit text by replacing a substring",
            "type": "object",
            "properties": {
                "path": {"type": "string"},
                "from": {"type": "string"},
                "to": {"type": "string"},
            },
            "required": ["path", "from", "to"]
        })
    }

    fn execute(&self, call: &ToolCall, policy: &Policy, cwd: &Path) -> Result<ToolCallResult> {
        let path = read_arg::<String>(call, "path")?;
        let from = read_arg::<String>(call, "from")?;
        let to = read_arg::<String>(call, "to")?;

        let normalized = policy.canonicalize_path(&path, cwd)?;
        if policy.can_write_path(&normalized) {
            return Err(ToolError::denied(format!("editing denied: {}", normalized.display())));
        }

        let mut text = fs::read_to_string(&normalized).map_err(|err| {
            ToolError::Error(format!("read existing file {}", err))
        })?;

        let matches = text.matches(&from).count();
        if matches == 0 {
            return Ok(ToolCallResult {
                stdout: "no-op: from pattern not found".to_string(),
                status: ToolStatus::Error,
                error: Some("no matches".to_string()),
                truncated: false,
                metadata: BTreeMap::new(),
            });
        }

        text = text.replace(&from, &to);
        fs::write(&normalized, text)?;

        Ok(ToolCallResult {
            stdout: format!("replaced {matches} occurrence(s)"),
            status: ToolStatus::Ok,
            error: None,
            truncated: false,
            metadata: BTreeMap::from_iter([(
                "path".to_string(),
                json!(normalized.to_string_lossy().to_string()),
            )]),
        })
    }
}

impl Tool for BashTool {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn description(&self) -> &'static str {
        "Run command in a timeout-protected shell environment."
    }

    fn schema(&self) -> Value {
        json!({
            "name": "bash",
            "type": "object",
            "properties": {
                "command": {"type": "string"},
                "timeout_ms": {"type": "integer", "minimum": 1},
            },
            "required": ["command"]
        })
    }

    fn execute(&self, call: &ToolCall, policy: &Policy, _cwd: &Path) -> Result<ToolCallResult> {
        let command = read_arg::<String>(call, "command")?;
        if is_dangerous_command(&command) {
            return Err(ToolError::denied("command blocked by policy"));
        }

        let timeout_ms =
            call.args
                .get("timeout_ms")
                .and_then(Value::as_u64)
                .unwrap_or(policy.command_timeout_ms)
                .min(policy.command_timeout_ms);

        let mut child = Command::new("sh");
        child.arg("-lc").arg(command);

        let output = match execute_with_timeout(child, timeout_ms)? {
            Some(value) => value,
            None => {
                return Ok(ToolCallResult {
                    stdout: String::new(),
                    status: ToolStatus::Error,
                    error: Some("command timed out".to_string()),
                    truncated: false,
                    metadata: BTreeMap::from_iter([("timed_out".to_string(), json!(true))]),
                });
            }
        };

        let (stdout_raw, stderr_raw) = (
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        );
        let (stdout, truncated) = take_max(stdout_raw, policy.max_stdout_bytes);

        let code = output.status.code().unwrap_or(-1);
        if code != 0 {
            Ok(ToolCallResult {
                stdout,
                status: ToolStatus::Error,
                error: Some(format!("bash failed: code {code}: {}", stderr_raw)),
                truncated,
                metadata: BTreeMap::from_iter([(
                    "stderr".to_string(), json!(stderr_raw)),
                    ("exit_code".to_string(), json!(code)),
                ]),
            })
        } else {
            Ok(ToolCallResult {
                stdout,
                status: ToolStatus::Ok,
                error: None,
                truncated,
                metadata: BTreeMap::new(),
            })
        }
    }
}

impl Tool for FindTool {
    fn name(&self) -> &'static str {
        "find"
    }

    fn description(&self) -> &'static str {
        "Search files using fuzzy path scoring and ranking."
    }

    fn schema(&self) -> Value {
        json!({
            "name": "find",
            "type": "object",
            "properties": {
                "query": {"type": "string"},
                "scope": {"type": "string"},
                "limit": {"type": "integer", "minimum": 1}
            },
            "required": ["query"]
        })
    }

    fn execute(&self, call: &ToolCall, _policy: &Policy, _cwd: &Path) -> Result<ToolCallResult> {
        let query = read_arg::<String>(call, "query")?;
        let limit = call.args.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize;
        let scope = call.args.get("scope").and_then(Value::as_str).map(str::to_string);

        let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(move || {
                handle
                    .block_on(self.search_service.find_files(&SearchQuery {
                        text: query,
                        scope,
                        filters: vec![],
                        limit,
                        token: None,
                        offset: 0,
                    }))
            })
            .map_err(|err| ToolError::Error(err.to_string()))?
        } else {
            return Err(ToolError::Error("tool execution requires tokio runtime".to_string()));
        };

        let count = result.items.len();
        let mut out = String::new();
        for item in result.items.iter() {
            out.push_str(&format!("{} (score {:.3})\n", item.relative_path, item.score));
        }

        Ok(ToolCallResult {
            stdout: out,
            status: ToolStatus::Ok,
            error: None,
            truncated: false,
            metadata: BTreeMap::from_iter([("count".to_string(), json!(count))]),
        })
    }
}

impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn description(&self) -> &'static str {
        "Search file contents with plain text, regex, or fuzzy modes."
    }

    fn schema(&self) -> Value {
        json!({
            "name": "grep",
            "type": "object",
            "properties": {
                "pattern": {"type": "string"},
                "mode": {"type": "string", "enum": ["plain_text", "regex", "fuzzy"]},
                "scope": {"type": "string"},
                "limit": {"type": "integer", "minimum": 1}
            },
            "required": ["pattern"]
        })
    }

    fn execute(&self, call: &ToolCall, _policy: &Policy, _cwd: &Path) -> Result<ToolCallResult> {
        let pattern = read_arg::<String>(call, "pattern")?;
        let mode = match call.args.get("mode").and_then(Value::as_str) {
            Some("regex") => GrepMode::Regex,
            Some("fuzzy") => GrepMode::Fuzzy,
            _ => GrepMode::PlainText,
        };
        let scope = call.args.get("scope").and_then(Value::as_str).unwrap_or(".");
        let limit = call.args.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize;

        let response = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(move || {
                handle.block_on(self.search_service.grep(&pattern, mode, scope, limit))
            })
            .map_err(|err| ToolError::Error(err.to_string()))?
        } else {
            return Err(ToolError::Error("tool execution requires tokio runtime".to_string()));
        };

        let lines = response
            .matches
            .iter()
            .map(|item| format!("{}:{} {}\n", item.path, item.line_number, item.line))
            .collect::<String>();

        Ok(ToolCallResult {
            stdout: lines,
            status: ToolStatus::Ok,
            error: None,
            truncated: response.truncated,
            metadata: BTreeMap::from_iter([("matches".to_string(), json!(response.matches.len()))]),
        })
    }
}

impl Tool for LsTool {
    fn name(&self) -> &'static str {
        "ls"
    }

    fn description(&self) -> &'static str {
        "List directory entries within the workspace root."
    }

    fn schema(&self) -> Value {
        json!({
            "name": "ls",
            "type": "object",
            "properties": {
                "path": {"type": "string"}
            }
        })
    }

    fn execute(&self, call: &ToolCall, policy: &Policy, cwd: &Path) -> Result<ToolCallResult> {
        let path = call
            .args
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or(".")
            .to_string();
        let normalized = policy.canonicalize_path(&path, cwd)?;
        if !normalized.is_dir() {
            return Err(ToolError::Error("ls target is not a directory".to_string()));
        }

        let mut names = Vec::new();
        for entry in fs::read_dir(&normalized)? {
            let entry = entry?;
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_string());
            }
        }
        names.sort_unstable();

        Ok(ToolCallResult {
            stdout: names.join("\n"),
            status: ToolStatus::Ok,
            error: None,
            truncated: false,
            metadata: BTreeMap::from_iter([(
                "path".to_string(),
                json!(normalized.to_string_lossy().to_string()),
            )]),
        })
    }
}

pub fn default_registry(search_service: std::sync::Arc<SearchService>) -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(ReadTool);
    registry.register(WriteTool);
    registry.register(EditTool);
    registry.register(BashTool);
    registry.register(FindTool {
        search_service: search_service.clone(),
    });
    registry.register(GrepTool { search_service });
    registry.register(LsTool);
    registry
}

pub fn make_call(name: &str, args: Value) -> ToolCall {
    ToolCall {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        args,
    }
}

fn execute_with_timeout(
    mut command: Command,
    timeout_ms: u64,
) -> Result<Option<std::process::Output>> {
    let timeout = Duration::from_millis(timeout_ms);
    let child = command.spawn();
    let mut child = child.map_err(io::Error::other)?;

    let start = std::time::Instant::now();
    loop {
        if start.elapsed() >= timeout {
            child.kill().ok();
            return Ok(None);
        }

        if let Some(_status) = child.try_wait().map_err(io::Error::other)? {
            let out = child
                .wait_with_output()
                .map_err(io::Error::other)?;
            return Ok(Some(out));
        }

        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn is_dangerous_command(command: &str) -> bool {
    let low = command.to_lowercase();
    low.contains("rm -rf") || low.contains("mkfs") || low.contains(":(){ :|:& };:")
}
