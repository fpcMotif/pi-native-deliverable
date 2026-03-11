#![forbid(unsafe_code)]

use pi_search::{GrepMode, GrepQuery, SearchQuery, SearchService};
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

/// Well-known rule IDs used in audit records and policy decisions.
pub const RULE_WORKSPACE_BOUNDARY: &str = "PT001";
pub const RULE_DENY_WRITE_PATH: &str = "PT002";
pub const RULE_BINARY_READ_BLOCKED: &str = "PT003";
pub const RULE_DANGEROUS_COMMAND: &str = "PT004";

/// Predefined security profiles for tool execution.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyPreset {
    Safe,
    Balanced,
    Permissive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolAuditRecord {
    pub call_id: String,
    pub tool: String,
    pub allowed: bool,
    pub rule_id: String,
    pub status: ToolStatus,
    pub error: Option<String>,
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

    pub fn execute(
        &self,
        name: &str,
        call: &ToolCall,
        policy: &Policy,
        cwd: &Path,
    ) -> Result<ToolCallResult> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| ToolError::not_found(name))?;
        tool.execute(call, policy, cwd)
    }

    pub fn execute_with_audit(
        &self,
        name: &str,
        call: &ToolCall,
        policy: &Policy,
        cwd: &Path,
        audit: &mut Vec<ToolAuditRecord>,
    ) -> Result<ToolCallResult> {
        let result = self.execute(name, call, policy, cwd);

        let (allowed, rule_id, status, error) = match &result {
            Ok(value) => (true, "tool.allow".to_string(), value.status.clone(), None),
            Err(error) => (
                false,
                tool_error_rule_id(error),
                ToolStatus::Denied,
                Some(format!("{error}")),
            ),
        };

        audit.push(ToolAuditRecord {
            call_id: call.id.clone(),
            tool: name.to_string(),
            allowed,
            rule_id,
            status,
            error,
        });

        result
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

    pub fn list_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
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
    pub preset: PolicyPreset,
    pub workspace_root: PathBuf,
    pub max_stdout_bytes: usize,
    pub max_stderr_bytes: usize,
    pub command_timeout_ms: u64,
    pub deny_write_paths: Vec<String>,
    pub max_file_size: usize,
}

impl Policy {
    pub fn safe_defaults(workspace_root: impl Into<PathBuf>) -> Self {
        Self::safe(workspace_root)
    }

    pub fn safe(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            preset: PolicyPreset::Safe,
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

    pub fn balanced(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            preset: PolicyPreset::Balanced,
            workspace_root: workspace_root.into(),
            max_stdout_bytes: 64 * 1024,
            max_stderr_bytes: 16 * 1024,
            command_timeout_ms: 10_000,
            deny_write_paths: vec![
                ".env".to_string(),
                ".env.local".to_string(),
                ".bash_history".to_string(),
            ],
            max_file_size: DEFAULT_WRITE_LIMIT_BYTES * 2,
        }
    }

    pub fn permissive(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            preset: PolicyPreset::Permissive,
            workspace_root: workspace_root.into(),
            max_stdout_bytes: 256 * 1024,
            max_stderr_bytes: 64 * 1024,
            command_timeout_ms: 30_000,
            deny_write_paths: Vec::new(),
            max_file_size: DEFAULT_WRITE_LIMIT_BYTES * 4,
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
            return Err(ToolError::invalid(
                "path",
                "parent directory does not exist",
            ));
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
        !path.components().any(|component| {
            if let Component::Normal(component) = component {
                let value = OsStr::to_string_lossy(component).to_lowercase();

                // Check direct matches from the deny list (case-insensitive)
                if self
                    .deny_write_paths
                    .iter()
                    .any(|deny| deny.to_lowercase() == value)
                {
                    return true;
                }

                // Hardcoded sensitive directories
                if value == ".git" || value == ".ssh" || value == ".aws" {
                    return true;
                }

                // Sensitive file prefixes (env files and SSH keys)
                if value.starts_with(".env") {
                    return true;
                }

                // Specific SSH key file patterns (not overly broad)
                if value == "id_rsa"
                    || value == "id_rsa.pub"
                    || value == "id_ed25519"
                    || value == "id_ed25519.pub"
                    || value == "id_ecdsa"
                    || value == "id_ecdsa.pub"
                    || value == "id_dsa"
                    || value == "id_dsa.pub"
                {
                    return true;
                }

                false
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

fn tool_error_rule_id(error: &ToolError) -> String {
    match error {
        ToolError::Denied(_) => "tool.policy.denied".to_string(),
        ToolError::InvalidArg(_) => "tool.input.invalid".to_string(),
        ToolError::NotFound(_) => "tool.missing".to_string(),
        ToolError::Error(_) | ToolError::Io(_) => "tool.execution.error".to_string(),
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
        .and_then(|value| {
            serde_json::from_value(value.clone())
                .map_err(|err| ToolError::invalid(name, err.to_string()))
        })
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
        let max = call
            .args
            .get("max_bytes")
            .and_then(|value| serde_json::from_value::<u64>(value.clone()).ok())
            .unwrap_or(policy.max_file_size as u64) as usize;

        let normalized = policy.canonicalize_path(&path, cwd)?;

        // Read first 8KB to check for binary content before reading the entire file
        const BINARY_CHECK_SIZE: usize = 8 * 1024;
        let mut file = fs::File::open(&normalized)?;
        let mut header = vec![0u8; BINARY_CHECK_SIZE];
        let header_len = file.read(&mut header)?;
        header.truncate(header_len);
        if header.contains(&0) {
            return Err(ToolError::denied("refusing to read binary file"));
        }

        // Read the rest of the file
        let header_end = header.len();
        let mut bytes = header;
        file.read_to_end(&mut bytes)?;
        if bytes.len() > header_end && bytes[header_end..].contains(&0) {
            return Err(ToolError::denied("refusing to read binary file"));
        }

        let (stdout, truncated) = take_max(String::from_utf8_lossy(&bytes).to_string(), max);
        Ok(ToolCallResult {
            stdout,
            status: ToolStatus::Ok,
            error: None,
            truncated,
            metadata: BTreeMap::from_iter([
                (
                    "path".to_string(),
                    json!(normalized.to_string_lossy().to_string()),
                ),
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
        let append = call
            .args
            .get("append")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        if content.len() > policy.max_file_size {
            return Err(ToolError::Error(format!(
                "content exceeds max size {}",
                policy.max_file_size
            )));
        }

        let normalized = policy.canonicalize_path(&path, cwd)?;
        if !policy.can_write_path(&normalized) {
            return Err(ToolError::denied(format!(
                "writing denied: {}",
                normalized.display()
            )));
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
        if !policy.can_write_path(&normalized) {
            return Err(ToolError::denied(format!(
                "editing denied: {}",
                normalized.display()
            )));
        }

        let mut text = fs::read_to_string(&normalized)
            .map_err(|err| ToolError::Error(format!("read existing file {}", err)))?;

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

        let timeout_ms = call
            .args
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
                metadata: BTreeMap::from_iter([
                    ("stderr".to_string(), json!(stderr_raw)),
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
        let scope = call
            .args
            .get("scope")
            .and_then(Value::as_str)
            .map(str::to_string);

        let result = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(move || {
                handle.block_on(self.search_service.find_files(&SearchQuery {
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
            return Err(ToolError::Error(
                "tool execution requires tokio runtime".to_string(),
            ));
        };

        let count = result.items.len();
        let mut out = String::new();
        for item in result.items.iter() {
            out.push_str(&format!(
                "{} (score {:.3})\n",
                item.relative_path, item.score
            ));
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
                "limit": {"type": "integer", "minimum": 1},
                "token": {"type": "string"}
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
        let scope = call
            .args
            .get("scope")
            .and_then(Value::as_str)
            .unwrap_or(".");
        let limit = call.args.get("limit").and_then(Value::as_u64).unwrap_or(50) as usize;
        let token = call
            .args
            .get("token")
            .and_then(Value::as_str)
            .map(|value| value.to_string());

        let response = if let Ok(handle) = tokio::runtime::Handle::try_current() {
            tokio::task::block_in_place(move || {
                handle.block_on(self.search_service.grep_query(GrepQuery {
                    pattern,
                    mode,
                    scope: scope.to_string(),
                    limit,
                    token,
                    offset: 0,
                }))
            })
            .map_err(|err| ToolError::Error(err.to_string()))?
        } else {
            return Err(ToolError::Error(
                "tool execution requires tokio runtime".to_string(),
            ));
        };

        let lines = response
            .matches
            .iter()
            .map(|item| format!("{}:{} {}\n", item.path, item.line_number, item.line))
            .collect::<String>();

        let mut metadata =
            BTreeMap::from_iter([("matches".to_string(), json!(response.matches.len()))]);
        if let Some(token) = response.token {
            let _ = metadata.insert("next_token".to_string(), json!(token));
        }
        if let Some(warning) = response.warning {
            let _ = metadata.insert("warning".to_string(), json!(warning));
        }

        Ok(ToolCallResult {
            stdout: lines,
            status: ToolStatus::Ok,
            error: None,
            truncated: response.truncated,
            metadata,
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
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    let child = command.spawn().map_err(io::Error::other)?;
    let (tx, rx) = std::sync::mpsc::channel();

    let pid = child.id();

    std::thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    match rx.recv_timeout(Duration::from_millis(timeout_ms)) {
        Ok(result) => {
            let out = result.map_err(io::Error::other)?;
            Ok(Some(out))
        }
        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
            #[cfg(unix)]
            {
                let _ = std::process::Command::new("kill")
                    .arg("-9")
                    .arg(pid.to_string())
                    .output();
            }
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .arg("/F")
                    .arg("/PID")
                    .arg(pid.to_string())
                    .output();
            }
            Ok(None)
        }
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
            Err(ToolError::Error("execute thread disconnected".into()))
        }
    }
}

pub fn is_dangerous_command(command: &str) -> bool {
    let low = command.to_lowercase();
    low.contains("rm -rf") || low.contains("mkfs") || low.contains(":(){ :|:& };:")
}

mod bash_test;
mod registry_test;
