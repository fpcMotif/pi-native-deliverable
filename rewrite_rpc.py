import re

with open('crates/pi-protocol/src/rpc.rs', 'r') as f:
    content = f.read()

# Define the new parse_client_request and the helper functions
new_code = """
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
        RpcEnvelopeType::Prompt => parse_prompt_request(raw_map)?,
        RpcEnvelopeType::Steer => parse_steer_request(raw_map)?,
        RpcEnvelopeType::FollowUp => parse_follow_up_request(raw_map)?,
        RpcEnvelopeType::Abort => parse_abort_request(raw_map)?,
        RpcEnvelopeType::GetState => parse_get_state_request(raw_map)?,
        RpcEnvelopeType::Compact => parse_compact_request(raw_map)?,
        RpcEnvelopeType::NewSession => parse_new_session_request(raw_map)?,
        RpcEnvelopeType::SelectSessionPath => parse_select_session_path_request(raw_map)?,
        RpcEnvelopeType::ForkSession => parse_fork_session_request(raw_map)?,
        RpcEnvelopeType::CheckoutBranchHead => parse_checkout_branch_head_request(raw_map)?,
        RpcEnvelopeType::Unknown => {
            return Err(ProtocolError::UnsupportedMessageType(
                raw_request_type.to_string(),
            ));
        }
    };

    Ok(request)
}

fn parse_prompt_request(raw_map: serde_json::Map<String, Value>) -> Result<ClientRequest, ProtocolError> {
    let request: PromptRequest = deserialize_request(Value::Object(raw_map))?;
    validate_version(&request.v)?;
    Ok(ClientRequest::Prompt {
        v: request.v,
        id: as_opt_string(request.id),
        message: request.message,
        attachments: request.attachments,
    })
}

fn parse_steer_request(raw_map: serde_json::Map<String, Value>) -> Result<ClientRequest, ProtocolError> {
    let request: MessageRequest = deserialize_request(Value::Object(raw_map))?;
    validate_version(&request.v)?;
    Ok(ClientRequest::Steer {
        v: request.v,
        id: as_opt_string(request.id),
        message: request.message,
    })
}

fn parse_follow_up_request(raw_map: serde_json::Map<String, Value>) -> Result<ClientRequest, ProtocolError> {
    let request: MessageRequest = deserialize_request(Value::Object(raw_map))?;
    validate_version(&request.v)?;
    Ok(ClientRequest::FollowUp {
        v: request.v,
        id: as_opt_string(request.id),
        message: request.message,
    })
}

fn parse_abort_request(raw_map: serde_json::Map<String, Value>) -> Result<ClientRequest, ProtocolError> {
    let request: IdOnlyRequest = deserialize_request(Value::Object(raw_map))?;
    validate_version(&request.v)?;
    Ok(ClientRequest::Abort {
        v: request.v,
        id: as_opt_string(request.id),
    })
}

fn parse_get_state_request(raw_map: serde_json::Map<String, Value>) -> Result<ClientRequest, ProtocolError> {
    let request: IdOnlyRequest = deserialize_request(Value::Object(raw_map))?;
    validate_version(&request.v)?;
    Ok(ClientRequest::GetState {
        v: request.v,
        id: as_opt_string(request.id),
    })
}

fn parse_compact_request(raw_map: serde_json::Map<String, Value>) -> Result<ClientRequest, ProtocolError> {
    let request: CompactRequest = deserialize_request(Value::Object(raw_map))?;
    validate_version(&request.v)?;
    Ok(ClientRequest::Compact {
        v: request.v,
        id: as_opt_string(request.id),
        reserve_tokens: request.reserve_tokens,
        keep_recent_tokens: request.keep_recent_tokens,
    })
}

fn parse_new_session_request(raw_map: serde_json::Map<String, Value>) -> Result<ClientRequest, ProtocolError> {
    let request: IdOnlyRequest = deserialize_request(Value::Object(raw_map))?;
    validate_version(&request.v)?;
    Ok(ClientRequest::NewSession {
        v: request.v,
        id: as_opt_string(request.id),
    })
}

fn parse_select_session_path_request(raw_map: serde_json::Map<String, Value>) -> Result<ClientRequest, ProtocolError> {
    let request: SessionPathRequest = deserialize_request(Value::Object(raw_map))?;
    validate_version(&request.v)?;
    Ok(ClientRequest::SelectSessionPath {
        v: request.v,
        id: as_opt_string(request.id),
        path: request.path,
    })
}

fn parse_fork_session_request(raw_map: serde_json::Map<String, Value>) -> Result<ClientRequest, ProtocolError> {
    let request: ForkSessionRequest = deserialize_request(Value::Object(raw_map))?;
    validate_version(&request.v)?;
    let from_turn_id = as_value_to_string(request.from_turn_id)
        .ok_or_else(|| ProtocolError::InvalidPayload("missing from_turn_id".to_string()))?;
    Ok(ClientRequest::ForkSession {
        v: request.v,
        id: as_opt_string(request.id),
        from_turn_id,
    })
}

fn parse_checkout_branch_head_request(raw_map: serde_json::Map<String, Value>) -> Result<ClientRequest, ProtocolError> {
    let request: CheckoutBranchHeadRequest = deserialize_request(Value::Object(raw_map))?;
    validate_version(&request.v)?;
    Ok(ClientRequest::CheckoutBranchHead {
        v: request.v,
        id: as_opt_string(request.id),
        from_turn_id: request.from_turn_id.and_then(as_value_to_string),
    })
}
"""

start_str = "pub fn parse_client_request(raw: &str) -> Result<ClientRequest, ProtocolError> {"
end_str = "fn as_opt_string(value: Option<Value>) -> Option<String> {"

start_idx = content.find(start_str)
end_idx = content.find(end_str)

if start_idx != -1 and end_idx != -1:
    content = content[:start_idx] + new_code + "\n" + content[end_idx:]
    with open('crates/pi-protocol/src/rpc.rs', 'w') as f:
        f.write(content)
    print("Replaced successfully")
else:
    print("Could not find start or end strings")
