#![forbid(unsafe_code)]

pub mod rpc;
pub mod session;

pub use rpc::{
    make_error_event, parse_client_request, to_json_line, to_jsonl_value, ProtocolError,
    ProtocolErrorPayload, ProtocolVersion, RpcLineCodecConfig, ServerEvent, ToJsonLineError,
};
pub use session::{normalize_jsonl, SessionEntry, SessionEntryKind, SessionLog, SessionQuery};

pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 0;
pub const PROTOCOL_PATCH: u16 = 0;
pub const PROTOCOL_VERSION: &str = "1.0.0";

pub fn protocol_version() -> String {
    format!("{}.{}.{}", PROTOCOL_MAJOR, PROTOCOL_MINOR, PROTOCOL_PATCH)
}

#[cfg(feature = "schema")]
pub use rpc::schema_json;
