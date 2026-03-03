# pi Native Rust Contract Index

## Protocol (`pi-protocol`)
- `parse_client_request` handles line-delimited JSON request parsing and version checks.
- `ClientRequest` supports the required command types (`prompt`, `steer`, `follow_up`, `abort`, `get_state`, `compact`, `new_session`).
- `ServerEvent` models response event streams and tracks request correlation IDs.
- `to_json_line` serializes server events as protocol lines.

## Sessions (`pi-session`)
- `SessionStore` provides append-only JSONL persistence.
- In-memory tree index is maintained via `entry_by_id`, `children`, and `roots`.
- Branch APIs include `branch_from`, `checkout`, `get_branch_head`, and `prune_to_depth`.
- Compaction and deterministic summary helpers are exposed.

## Search (`pi-search`)
- `SearchService` exposes `find_files` and `grep` APIs.
- Query and response types: `SearchQuery`, `SearchResponse`, `SearchStats`.
- Grep modes: `PlainText`, `Regex`, `Fuzzy`.

## Tools (`pi-tools`)
- Policy-gated execution surface: `read`, `write`, `edit`, `bash`, `grep`, `find`, `ls`.
- A shared schema is produced per tool and exported by registry.
- Default policy denies common secret paths.

## Runtime (`pi-core`)
- `Agent` owns session state, provider stream consumption, and tool-call execution.
- `run_rpc` handles stdin/stdout framed by line-delimited JSON events.
