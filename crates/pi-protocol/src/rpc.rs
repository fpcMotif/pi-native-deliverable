#![forbid(unsafe_code)]

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use thiserror::Error;

use crate::{PROTOCOL_MAJOR, PROTOCOL_MINOR, PROTOCOL_VERSION};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct RpcLineCodecConfig {
    pub max_line_bytes: usize,
}

impl Default for RpcLineCodecConfig {
    fn default() -> Self {
        Self {
            max_line_bytes: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl ProtocolVersion {
    pub fn parse(value: &str) -> Result<Self, ProtocolError> {
        let mut parts = value.split('.');
        let major = parts
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| ProtocolError::InvalidVersion(value.to_string()))?;
        let minor = parts
            .next()
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| ProtocolError::InvalidVersion(value.to_string()))?;
        let patch = {
            let patch = parts
                .next()
                .ok_or_else(|| ProtocolError::InvalidVersion(value.to_string()))?;
            if patch == "x" || patch == "*" {
                0
            } else {
                patch
                    .parse()
                    .map_err(|_| ProtocolError::InvalidVersion(value.to_string()))?
            }
        };
        if parts.next().is_some() {
            return Err(ProtocolError::InvalidVersion(value.to_string()));
        }
        Ok(Self {
            major,
            minor,
            patch,
        })
    }

    pub fn is_compatible(&self) -> bool {
        self.major == PROTOCOL_MAJOR && self.minor == PROTOCOL_MINOR
    }

    pub fn as_string(&self) -> String {
        format!("{}.{}.{}", self.major, self.minor, self.patch)
    }
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ClientRequest {
    Prompt {
        v: String,
        id: Option<String>,
        message: String,
        attachments: Option<Vec<Value>>,
    },
    Steer {
        v: String,
        id: Option<String>,
        message: String,
    },
    FollowUp {
        v: String,
        id: Option<String>,
        message: String,
    },
    Abort {
        v: String,
        id: Option<String>,
    },
    GetState {
        v: String,
        id: Option<String>,
    },
    Compact {
        v: String,
        id: Option<String>,
        reserve_tokens: Option<u32>,
        keep_recent_tokens: Option<u32>,
    },
    NewSession {
        v: String,
        id: Option<String>,
    },
    SelectSessionPath {
        v: String,
        id: Option<String>,
        path: String,
    },
    ForkSession {
        v: String,
        id: Option<String>,
        from_turn_id: String,
    },
    CheckoutBranchHead {
        v: String,
        id: Option<String>,
        from_turn_id: Option<String>,
    },
    Unknown {
        v: String,
        id: Option<String>,
        request_type: String,
        payload: Value,
    },
}

impl ClientRequest {
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Prompt { id, .. }
            | Self::Steer { id, .. }
            | Self::FollowUp { id, .. }
            | Self::Abort { id, .. }
            | Self::GetState { id, .. }
            | Self::Compact { id, .. }
            | Self::NewSession { id, .. }
            | Self::SelectSessionPath { id, .. }
            | Self::ForkSession { id, .. }
            | Self::CheckoutBranchHead { id, .. }
            | Self::Unknown { id, .. } => id.as_deref(),
        }
    }

    pub fn protocol_version(&self) -> &str {
        match self {
            Self::Prompt { v, .. }
            | Self::Steer { v, .. }
            | Self::FollowUp { v, .. }
            | Self::Abort { v, .. }
            | Self::GetState { v, .. }
            | Self::Compact { v, .. }
            | Self::NewSession { v, .. }
            | Self::SelectSessionPath { v, .. }
            | Self::ForkSession { v, .. }
            | Self::CheckoutBranchHead { v, .. }
            | Self::Unknown { v, .. } => v.as_str(),
        }
    }

    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Prompt { message, .. } => Some(message),
            Self::Steer { message, .. } => Some(message),
            Self::FollowUp { message, .. } => Some(message),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ServerEventKind {
    Ready,
    Error,
    TurnStart,
    TurnEnd,
    MessageUpdate,
    ToolCallStarted,
    ToolCallResult,
    ToolCallError,
    State,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ProtocolErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(default)]
    pub details: Option<Value>,
}

impl ProtocolErrorPayload {
    pub fn new(
        code: impl Into<String>,
        message: impl Into<String>,
        details: Option<Value>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ServerEvent {
    Ready {
        v: String,
        id: Option<String>,
        request_id: Option<String>,
        capabilities: Value,
    },
    Error {
        v: String,
        id: Option<String>,
        request_id: Option<String>,
        error: ProtocolErrorPayload,
    },
    TurnStart {
        v: String,
        id: Option<String>,
        request_id: Option<String>,
        kind: String,
    },
    TurnEnd {
        v: String,
        id: Option<String>,
        request_id: Option<String>,
        reason: Option<String>,
    },
    MessageUpdate {
        v: String,
        id: Option<String>,
        request_id: Option<String>,
        delta: String,
        done: bool,
    },
    ToolCallStarted {
        v: String,
        id: Option<String>,
        request_id: Option<String>,
        tool_name: String,
        call_id: String,
        args: Value,
    },
    ToolCallResult {
        v: String,
        id: Option<String>,
        request_id: Option<String>,
        tool_name: String,
        call_id: String,
        output: Value,
    },
    ToolCallError {
        v: String,
        id: Option<String>,
        request_id: Option<String>,
        tool_name: String,
        call_id: String,
        error: ProtocolErrorPayload,
    },
    State {
        v: String,
        id: Option<String>,
        request_id: Option<String>,
        payload: Value,
    },
}

impl ServerEvent {
    pub fn with_request_id(mut self, request_id: Option<String>) -> Self {
        match &mut self {
            Self::Ready {
                request_id: rid, ..
            }
            | Self::Error {
                request_id: rid, ..
            }
            | Self::TurnStart {
                request_id: rid, ..
            }
            | Self::TurnEnd {
                request_id: rid, ..
            }
            | Self::MessageUpdate {
                request_id: rid, ..
            }
            | Self::ToolCallStarted {
                request_id: rid, ..
            }
            | Self::ToolCallResult {
                request_id: rid, ..
            }
            | Self::ToolCallError {
                request_id: rid, ..
            }
            | Self::State {
                request_id: rid, ..
            } => *rid = request_id,
        }
        self
    }

    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Ready { request_id, .. }
            | Self::Error { request_id, .. }
            | Self::TurnStart { request_id, .. }
            | Self::TurnEnd { request_id, .. }
            | Self::MessageUpdate { request_id, .. }
            | Self::ToolCallStarted { request_id, .. }
            | Self::ToolCallResult { request_id, .. }
            | Self::ToolCallError { request_id, .. }
            | Self::State { request_id, .. } => request_id.as_deref(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid version: {0}")]
    InvalidVersion(String),
    #[error("unsupported message type: {0}")]
    UnsupportedMessageType(String),
    #[error("invalid payload: {0}")]
    InvalidPayload(String),
    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug)]
pub enum ToJsonLineError {
    Serialize(serde_json::Error),
}

impl fmt::Display for ToJsonLineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ToJsonLineError {}

pub fn to_jsonl_value(event: &ServerEvent) -> String {
    let mut value = serde_json::to_value(event).expect("serialize event");
    if let Value::Object(map) = &mut value {
        map.insert(
            "type".to_string(),
            Value::String(server_event_type(event).to_string()),
        );
        map.insert("v".to_string(), Value::String(PROTOCOL_VERSION.to_string()));
    }
    serde_json::to_string(&value).expect("serialize event to string")
}

pub fn to_json_line(event: &ServerEvent) -> Result<String, ToJsonLineError> {
    let mut value = serde_json::to_value(event).map_err(ToJsonLineError::Serialize)?;
    if let Value::Object(map) = &mut value {
        map.insert(
            "type".to_string(),
            Value::String(server_event_type(event).to_string()),
        );
        map.insert("v".to_string(), Value::String(PROTOCOL_VERSION.to_string()));
    }
    serde_json::to_string(&value)
        .map_err(ToJsonLineError::Serialize)
        .map(|line| format!("{line}\n"))
}

#[derive(Debug)]
enum RpcEnvelopeType {
    Prompt,
    Steer,
    FollowUp,
    Abort,
    GetState,
    Compact,
    NewSession,
    SelectSessionPath,
    ForkSession,
    CheckoutBranchHead,
    Unknown,
}

pub fn make_error_event(
    code: impl Into<String>,
    message: impl Into<String>,
    request_id: Option<String>,
) -> ServerEvent {
    ServerEvent::Error {
        v: PROTOCOL_VERSION.to_string(),
        id: None,
        request_id,
        error: ProtocolErrorPayload::new(code, message, None),
    }
}

#[derive(Deserialize)]
struct PromptRequest {
    v: String,
    id: Option<serde_json::Value>,
    message: String,
    attachments: Option<Vec<Value>>,
}

#[derive(Deserialize)]
struct MessageRequest {
    v: String,
    id: Option<serde_json::Value>,
    message: String,
}

#[derive(Deserialize)]
struct IdOnlyRequest {
    v: String,
    id: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct CompactRequest {
    v: String,
    id: Option<serde_json::Value>,
    reserve_tokens: Option<u32>,
    keep_recent_tokens: Option<u32>,
}

#[derive(Deserialize)]
struct SessionPathRequest {
    v: String,
    id: Option<serde_json::Value>,
    path: String,
}

#[derive(Deserialize)]
struct ForkSessionRequest {
    v: String,
    id: Option<serde_json::Value>,
    from_turn_id: serde_json::Value,
}

#[derive(Deserialize)]
struct CheckoutBranchHeadRequest {
    v: String,
    id: Option<serde_json::Value>,
    from_turn_id: Option<serde_json::Value>,
}

pub fn parse_client_request(raw: &str) -> Result<ClientRequest, ProtocolError> {
    let envelope = serde_json::from_str::<Value>(raw)?;
    let Value::Object(mut raw_map) = envelope else {
        return Err(ProtocolError::InvalidPayload(
            "request is not an object".to_string(),
        ));
    };

    let raw_request_type = raw_map
        .remove("type")
        .and_then(|value| value.as_str().map(str::to_string))
        .ok_or_else(|| ProtocolError::InvalidPayload("missing type".to_string()))?
        .to_ascii_lowercase();

    let request_type = raw_request_type.as_str();
    validate_version(
        raw_map
            .get("v")
            .and_then(Value::as_str)
            .ok_or_else(|| ProtocolError::InvalidVersion(PROTOCOL_VERSION.to_string()))?,
    )?;

    let request_type = match request_type {
        "prompt" => RpcEnvelopeType::Prompt,
        "steer" => RpcEnvelopeType::Steer,
        "follow_up" => RpcEnvelopeType::FollowUp,
        "abort" => RpcEnvelopeType::Abort,
        "get_state" => RpcEnvelopeType::GetState,
        "compact" => RpcEnvelopeType::Compact,
        "new_session" => RpcEnvelopeType::NewSession,
        "select_session_path" => RpcEnvelopeType::SelectSessionPath,
        "fork_session" => RpcEnvelopeType::ForkSession,
        "checkout_branch_head" => RpcEnvelopeType::CheckoutBranchHead,
        _ => RpcEnvelopeType::Unknown,
    };

    let request = match request_type {
        RpcEnvelopeType::Prompt => {
            let request: PromptRequest = deserialize_request(Value::Object(raw_map.clone()))?;
            validate_version(&request.v)?;
            ClientRequest::Prompt {
                v: request.v,
                id: as_opt_string(request.id),
                message: request.message,
                attachments: request.attachments,
            }
        }
        RpcEnvelopeType::Steer => {
            let request: MessageRequest = deserialize_request(Value::Object(raw_map.clone()))?;
            validate_version(&request.v)?;
            ClientRequest::Steer {
                v: request.v,
                id: as_opt_string(request.id),
                message: request.message,
            }
        }
        RpcEnvelopeType::FollowUp => {
            let request: MessageRequest = deserialize_request(Value::Object(raw_map.clone()))?;
            validate_version(&request.v)?;
            ClientRequest::FollowUp {
                v: request.v,
                id: as_opt_string(request.id),
                message: request.message,
            }
        }
        RpcEnvelopeType::Abort => {
            let request: IdOnlyRequest = deserialize_request(Value::Object(raw_map.clone()))?;
            validate_version(&request.v)?;
            ClientRequest::Abort {
                v: request.v,
                id: as_opt_string(request.id),
            }
        }
        RpcEnvelopeType::GetState => {
            let request: IdOnlyRequest = deserialize_request(Value::Object(raw_map.clone()))?;
            validate_version(&request.v)?;
            ClientRequest::GetState {
                v: request.v,
                id: as_opt_string(request.id),
            }
        }
        RpcEnvelopeType::Compact => {
            let request: CompactRequest = deserialize_request(Value::Object(raw_map.clone()))?;
            validate_version(&request.v)?;
            ClientRequest::Compact {
                v: request.v,
                id: as_opt_string(request.id),
                reserve_tokens: request.reserve_tokens,
                keep_recent_tokens: request.keep_recent_tokens,
            }
        }
        RpcEnvelopeType::NewSession => {
            let request: IdOnlyRequest = deserialize_request(Value::Object(raw_map.clone()))?;
            validate_version(&request.v)?;
            ClientRequest::NewSession {
                v: request.v,
                id: as_opt_string(request.id),
            }
        }
        RpcEnvelopeType::SelectSessionPath => {
            let request: SessionPathRequest = deserialize_request(Value::Object(raw_map.clone()))?;
            validate_version(&request.v)?;
            ClientRequest::SelectSessionPath {
                v: request.v,
                id: as_opt_string(request.id),
                path: request.path,
            }
        }
        RpcEnvelopeType::ForkSession => {
            let request: ForkSessionRequest = deserialize_request(Value::Object(raw_map.clone()))?;
            validate_version(&request.v)?;
            let from_turn_id = as_value_to_string(request.from_turn_id)
                .ok_or_else(|| ProtocolError::InvalidPayload("missing from_turn_id".to_string()))?;
            ClientRequest::ForkSession {
                v: request.v,
                id: as_opt_string(request.id),
                from_turn_id,
            }
        }
        RpcEnvelopeType::CheckoutBranchHead => {
            let request: CheckoutBranchHeadRequest =
                deserialize_request(Value::Object(raw_map.clone()))?;
            validate_version(&request.v)?;
            ClientRequest::CheckoutBranchHead {
                v: request.v,
                id: as_opt_string(request.id),
                from_turn_id: request.from_turn_id.and_then(as_value_to_string),
            }
        }
        RpcEnvelopeType::Unknown => {
            return Err(ProtocolError::UnsupportedMessageType(
                raw_request_type.to_string(),
            ));
        }
    };

    Ok(request)
}

fn as_opt_string(value: Option<Value>) -> Option<String> {
    value.and_then(as_value_to_string)
}

fn as_value_to_string(value: Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn validate_version(value: &str) -> Result<(), ProtocolError> {
    let parsed = ProtocolVersion::parse(value)?;
    if !parsed.is_compatible() {
        return Err(ProtocolError::InvalidVersion(value.to_string()));
    }
    Ok(())
}

fn deserialize_request<T: DeserializeOwned>(value: Value) -> Result<T, ProtocolError> {
    serde_json::from_value(value).map_err(|err| ProtocolError::InvalidPayload(err.to_string()))
}

fn server_event_type(event: &ServerEvent) -> &'static str {
    match event {
        ServerEvent::Ready { .. } => "ready",
        ServerEvent::Error { .. } => "error",
        ServerEvent::TurnStart { .. } => "turn_start",
        ServerEvent::TurnEnd { .. } => "turn_end",
        ServerEvent::MessageUpdate { .. } => "message_update",
        ServerEvent::ToolCallStarted { .. } => "tool_call_started",
        ServerEvent::ToolCallResult { .. } => "tool_call_result",
        ServerEvent::ToolCallError { .. } => "tool_call_error",
        ServerEvent::State { .. } => "state",
    }
}

#[cfg(feature = "schema")]
pub fn schema_json() -> Value {
    let envelope_request = serde_json::json!({
        "client_request": {
            "v": PROTOCOL_VERSION,
            "type": ["prompt", "steer", "follow_up", "abort", "get_state", "compact", "new_session", "select_session_path", "fork_session", "checkout_branch_head"],
            "id": "string"
        },
        "server_event": {
            "ready": {"capabilities": "object"},
            "error": {"error": "ProtocolErrorPayload"},
            "message_update": {"delta": "string", "done": "bool"},
            "tool_call_started": {"tool_name": "string", "call_id": "string"},
            "tool_call_result": {"tool_name": "string", "call_id": "string", "output": "object"},
            "tool_call_error": {"tool_name": "string", "call_id": "string", "error": "ProtocolErrorPayload"},
            "state": {"payload": "object"}
        }
    });
    envelope_request
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_client_request_not_object() {
        let result = parse_client_request("[]");
        assert!(result.is_err());

        match result.expect_err("expected an error when parsing an array") {
            ProtocolError::InvalidPayload(msg) => {
                assert_eq!(msg, "request is not an object");
            }
            err => panic!("Expected ProtocolError::InvalidPayload, got: {:?}", err),
        }
    }

    #[test]
    fn test_parse_client_request_invalid_json() {
        let result = parse_client_request("{");
        assert!(result.is_err());

        match result.expect_err("expected an error when parsing invalid JSON") {
            ProtocolError::Json(_) => {}
            err => panic!("Expected ProtocolError::Json, got: {:?}", err),
        }
    }

    #[test]
    fn test_to_jsonl_value_ready_event() {
        let event = ServerEvent::Ready {
            v: "1.0.0".to_string(),
            id: Some("req-123".to_string()),
            request_id: Some("req-123".to_string()),
            capabilities: serde_json::json!({ "feature": true }),
        };

        let result = to_jsonl_value(&event);
        let parsed: Value = serde_json::from_str(&result).expect("failed to parse JSON string");

        assert_eq!(parsed["type"], "ready");
        assert_eq!(parsed["v"], PROTOCOL_VERSION);
    }

    #[test]
    fn test_to_jsonl_value_error_event() {
        let event = ServerEvent::Error {
            v: "1.0.0".to_string(),
            id: None,
            request_id: Some("req-456".to_string()),
            error: ProtocolErrorPayload {
                code: "invalid_request".to_string(),
                message: "Missing parameter".to_string(),
                details: None,
            },
        };

        let result = to_jsonl_value(&event);
        let parsed: Value = serde_json::from_str(&result).expect("failed to parse JSON string");

        assert_eq!(parsed["type"], "error");
        assert_eq!(parsed["v"], PROTOCOL_VERSION);
    }

    #[test]
    fn test_to_jsonl_value_turn_start_event() {
        let event = ServerEvent::TurnStart {
            v: "1.0.0".to_string(),
            id: Some("req-789".to_string()),
            request_id: Some("req-789".to_string()),
            kind: "chat".to_string(),
        };

        let result = to_jsonl_value(&event);
        let parsed: Value = serde_json::from_str(&result).expect("failed to parse JSON string");

        assert_eq!(parsed["type"], "turn_start");
        assert_eq!(parsed["v"], PROTOCOL_VERSION);
    }

    #[test]
    fn test_to_jsonl_value_message_update_event() {
        let event = ServerEvent::MessageUpdate {
            v: "1.0.0".to_string(),
            id: Some("req-101".to_string()),
            request_id: Some("req-101".to_string()),
            delta: "Hello".to_string(),
            done: false,
        };

        let result = to_jsonl_value(&event);
        let parsed: Value = serde_json::from_str(&result).expect("failed to parse JSON string");

        assert_eq!(parsed["type"], "message_update");
        assert_eq!(parsed["v"], PROTOCOL_VERSION);
    }
}
