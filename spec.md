# pi-native-rs Technical Specification

## 0. Scope and intent

This document specifies a **from-scratch, Rust-native rewrite** of the Pi agent CLI + toolkit, consolidating the goals and surface areas from:

- **pi_agent_rust** (Rust CLI port: interactive/print/RPC modes, JSONL session tree, built-in tools, capability-gated extensions).  
- **pi-mono** (toolkit: coding-agent CLI, unified LLM API, SDK, extensibility via extensions/skills/prompts/themes/packages).  
- **fff.nvim** (high-performance Rust-backed fuzzy file picker + grep with persistent index, frecency, git status integration, and typo-resistant search).

The primary design goal is **idiomatic Rust**: memory safety, predictable performance, clear ownership boundaries, structured concurrency, and a testable architecture with strong contracts.

---

## 1. Product goals (engineering translation)

### 1.1 Must-haves

1. **Single binary CLI**: `pi` with interactive TUI, print mode, and headless RPC mode.
2. **Unified LLM API**: provider-agnostic streaming interface with tool calling, model registry, token/cost tracking.
3. **Sessions**: append-only event log, branching tree, compaction, deterministic export/import.
4. **Tools**: `read/write/edit/bash/grep/find/ls` built-ins; policy-gated; safe-by-default.
5. **Search**: persistent file index + typo-tolerant fuzzy path search; fast grep (plain/regex/fuzzy) with highlights.
6. **Extensibility**: skills/prompts/themes/packages + extension runtime with capability-based isolation.

### 1.2 Non-goals

- Full IDE replacement.
- Default “swarm of sub-agents” behavior.
- Building a proprietary LLM gateway (support it, don’t require it).

---

## 2. Repository layout (Rust workspace)

```
pi-native-rs/
  Cargo.toml                    # workspace
  crates/
    pi-cli/                     # `pi` binary (TUI + print + rpc)
    pi-core/                    # agent state machine, session, scheduler
    pi-protocol/                # RPC + on-disk schemas (serde types + jsonschema)
    pi-config/                  # settings + env overrides + migration
    pi-llm/                     # providers + streaming + tool-calling parser
    pi-tools/                   # built-in tools + policy enforcement
    pi-search/                  # persistent index + fuzzy file search + grep engine
    pi-ext/                     # extension runtime(s): WASM (primary) + JS compat (optional) + MCP bridge
    pi-ui-tui/                  # TUI widgets, renderer, completion
    pi-ui-web/                  # optional: API server + thin web UI assets
    pi-bot-slack/               # optional: slack bot
    pi-pods/                    # optional: vLLM pod templates + docs
  tests/                        # cross-crate integration tests (black-box)
  fixtures/                     # small repos & corpus for deterministic tests
  docs/
    spec.md                     # this file
    prd.json
```

### 2.1 Dependency policy

- All workspace crates: `#![forbid(unsafe_code)]`.
- Use best-in-class ecosystem crates (tokio/reqwest/serde/tracing/etc.) as dependencies are allowed to contain internal `unsafe`.
- Prefer deterministic behavior: avoid non-deterministic iteration ordering in user-visible surfaces.

---

## 3. Core concepts & data model

### 3.1 AgentSession

`AgentSession` is the primary stateful controller:

- Holds message history, system prompt, tool registry, model selection.
- Produces a unified stream of events for any UI surface:
  - `message_start`, `message_update`, `message_end`
  - `tool_execution_start/update/end`
  - `turn_start/end`
  - `auto_compaction_start/end`
  - `auto_retry_start/end`
  - `error`

**Key constraint:** UI rendering must not block agent progress; all I/O is async.

### 3.2 Session storage: append-only event log + optional sidecar

**Primary format: JSON Lines (`.jsonl`)** with explicit schema versions.

- Advantages: diffable, append-only, easy recovery, simple streaming export.
- Each entry is a single JSON object with:
  - `schema_version`
  - `entry_id` (UUID v7 preferred; v4 acceptable)
  - `timestamp_ms`
  - `kind` (enum)
  - `parent_id` (for tree)
  - `payload` (typed per kind)

Optional **sidecar DB** (SQLite or LMDB) for indexing:
- session index, full-text excerpt index, attachments lookup, compaction cache.

#### 3.2.1 Entry kinds

- `system_prompt_set`
- `user_message`
- `assistant_message`
- `tool_call`
- `tool_result`
- `model_change`
- `thinking_level_change`
- `compaction_snapshot`
- `session_fork`
- `session_metadata` (tokens, costs, timings)
- `extension_event` (audit/log)

### 3.3 Configuration

- Global settings: `~/.pi/agent/settings.json` (compat with existing Pi expectations).
- Project settings: `.pi/settings.json` (override/extend).
- Environment overrides: `PI_CODING_AGENT_DIR`, `PI_SESSIONS_DIR`, `PI_PACKAGE_DIR`, etc.

The config loader produces a single resolved `EffectiveConfig` with provenance:
- for each setting: source + path + override precedence.

---

## 4. CLI surfaces

### 4.1 Modes

1. **Interactive (default)**  
   `pi [initial prompt]`  
   - TUI, streaming, completion, slash commands, tool panels.

2. **Print**  
   `pi -p "..."`  
   - one-shot response to stdout, no UI.

3. **RPC**  
   `pi --mode rpc`  
   - headless line-delimited JSON over stdin/stdout (for IDEs, bots, scripts).

### 4.2 Command palette and slash commands

Slash commands are first-class (rendered in completion list). Minimum built-ins:

- `/help`
- `/model`
- `/tree`
- `/clear`
- `/compact`
- `/exit`
- `/reload` (reload extensions/skills/prompts/themes)

---

## 5. RPC protocol (pi-protocol)

### 5.1 Framing

- UTF-8, one JSON object per line.
- Every message includes:
  - `v`: protocol version (semver string)
  - `type`: message type
  - `id`: optional request id for correlation

### 5.2 Client → server commands

- `prompt`: `{type:"prompt", message:string, id?:string, attachments?:[...] }`
- `steer`: `{type:"steer", message:string}`
- `follow_up`: `{type:"follow_up", message:string}`
- `abort`: `{type:"abort"}`
- `get_state`: `{type:"get_state"}`
- `compact`: `{type:"compact", reserve_tokens?:u32, keep_recent_tokens?:u32}`

### 5.3 Server → client events

- `ready`
- `error` (structured)
- `message_update` (text/thinking deltas)
- `tool_execution_*`
- `turn_*`
- `state` (snapshot)

### 5.4 Compatibility rules

- PATCH: additive fields only.
- MINOR: additive message types allowed; must not break existing.
- MAJOR: breaking changes allowed with migration doc + test fixtures.

---

## 6. Unified LLM API (pi-llm)

### 6.1 Traits and types

```rust
pub trait Provider: Send + Sync {
  fn name(&self) -> &'static str;
  fn list_models(&self) -> Result<Vec<ModelCard>>;
  fn stream(&self, req: CompletionRequest) -> BoxStream<'static, ProviderEvent>;
}
```

`ProviderEvent` includes:
- `TextDelta(String)`
- `ThinkingDelta(String)`
- `ToolCallDelta { tool_name, json_fragment }`
- `Stop { reason }`
- `Usage { input_tokens, output_tokens, cached_tokens, cost_usd }`

### 6.2 Tool calling

- Tools are defined with:
  - name
  - description
  - JSON schema for arguments
  - executor function returning `ToolResult` (streamable optional)

### 6.3 Streaming tool arguments (partial JSON)

Implement incremental decoding:
- Collect partial JSON fragments.
- Validate once complete (jsonschema).
- If invalid:
  - raise `tool_argument_error`, include raw buffer, and ask model to retry with correction.

### 6.4 Model registry

A `ModelRegistry` composes:
- compiled-in provider metadata
- user custom models (models.json)
- runtime discovery (where providers support it)

---

## 7. Tools (pi-tools)

### 7.1 Built-ins

- `read`: file read (optionally images)
- `write`: create/overwrite file
- `edit`: surgical replacement / patch application
- `bash`: run shell command with timeout
- `grep`: content search with context + highlights
- `find`: file discovery by pattern or fuzzy query
- `ls`: list directory

### 7.2 Policy enforcement

Policies are evaluated before execution:

- File policies:
  - allowlist roots
  - denylist globs (e.g., `.env`, secrets)
  - max file size
- Bash policies:
  - deny interactive flags
  - timeout + kill process group
  - optional command allowlist
- Network policies:
  - off by default for extensions
  - tool-level network (if any) requires explicit opt-in

Every decision yields an **audit record**:
- allowed/denied
- policy rule matched
- hashes of arguments (optional)
- timestamp

---

## 8. Search subsystem (pi-search)

This is the biggest planned improvement. We explicitly borrow design patterns from **fff.nvim**:

- Dedicated background runtime keeps a file index, tracks access/modification, git status, and serves typo-resistant fuzzy search with very low latency.
- Grep engine supports plain/regex/fuzzy and returns highlight spans.

### 8.1 Components

1. **Indexer**
   - Walk the repo using `ignore::WalkBuilder` (respects `.gitignore`).
   - Store file list + metadata in persistent store.
   - Watch changes using `notify` debouncer; apply incremental updates.

2. **Metadata**
   - `FileItem`:
     - absolute path, relative path
     - file name
     - size
     - last modified time
     - `is_binary` (fast NUL heuristic)
     - git status (optional)
     - frecency scores (access + modification)

3. **Persistent store**
   - LMDB via `heed` (fast reads, compact footprint).
   - Stores:
     - file table keyed by path hash
     - frecency events
     - query history and query→file association (combo boosts)
     - git status cache (optional)

4. **Query parser**
   - Supports:
     - fuzzy path parts
     - constraints/filters (e.g., `ext:rs`, `type:rust`, `is:modified`, `path:src/`)
     - location suffix parsing (`file:line:col` like UX)

5. **Fuzzy matcher**
   - Use typo-tolerant scoring (neo_frizbee-style) with:
     - `max_typos = clamp(len/4, 2..6)`
     - uppercase-sensitive bonuses
     - multi-part matching (Nucleo-like sum of scores)

6. **Scorer**
   - Total score:
     - `base_score` (fuzzy path match)
     - + frecency boost (access + modification, git-aware)
     - + filename bonus
     - + entrypoint bonus (`main.rs`, `lib.rs`, `index.ts`, etc.)
     - + combo boost (query→file learned)
     - + distance penalty (from current file)
     - − current file penalty (downrank open file)

7. **Grep engine**
   - Plain: SIMD substring via `memchr::memmem`.
   - Regex: bytes-regex; error captured and can fall back.
   - Fuzzy: per-line smith-waterman-like scoring; returns highlight spans.

### 8.2 APIs

`pi-search` exposes:

- `SearchService::fuzzy_files(query, ctx) -> SearchResult`
- `SearchService::grep(query, opts) -> GrepResult`
- `SearchService::complete_paths(prefix) -> Vec<CompletionItem>`

### 8.3 Completion integration

`pi-ui-tui` uses the search service for:
- `@` file reference completion
- `/` slash command completion
- skill/prompt completion

**Important:** completion must not block input. Design:
- UI sends query to search service over channel.
- Search replies within a frame budget; stale results acceptable.

---

## 9. Extensions (pi-ext)

### 9.1 Runtime families

1. **WASM component model (primary)**
   - Extensions compiled to WASM components.
   - Host exposes WIT-defined hostcalls:
     - tool invocation
     - UI prompts (interactive only)
     - session append/read
     - HTTP fetch (gated)
   - Capability policy enforced at hostcall boundary.

2. **JS/TS compatibility runtime (optional)**
   - Embedded JS engine (QuickJS-style) with:
     - no Node/Bun requirement
     - shims for required APIs
     - deterministic event loop contract
   - Used to preserve existing extension ecosystem.

3. **MCP bridge**
   - Import external tools via MCP protocol.
   - Tools become first-class in tool registry.

### 9.2 Extension lifecycle

- Load: discover extensions from global/project/package locations.
- Validate: manifest + declared capabilities.
- Activate: register tools/commands/hooks.
- Hot reload: invalidate + reload on `/reload` or file changes.

### 9.3 Hook points

- before/after tool call
- before/after message
- compaction customization
- rendering customization (interactive mode)

---

## 10. Skills, prompts, themes, packages

### 10.1 Skills

- Implement Agent Skills standard (directory with `SKILL.md` and frontmatter).
- Progressive disclosure:
  - only name + description always in system prompt
  - full skill loaded on-demand.

### 10.2 Prompt templates

- File-based templates expand to content before sending.
- Templates can include variables:
  - `{{cwd}}`, `{{selection}}`, `{{clipboard}}`, etc.

### 10.3 Themes

- JSON theme specs for TUI (colors, layout).
- Validate on startup; fallback to default.

### 10.4 Packages

- Discover resources from:
  - filesystem directories
  - git checkouts
  - (optional) npm packages (via external resolver)

---

## 11. Observability

- `tracing` spans for:
  - provider requests
  - tool execution
  - search queries
  - extension hostcalls
- Optional OpenTelemetry exporter.

---

## 12. Error handling

- No panics in normal operation; errors are surfaced as:
  - structured event to UI/RPC
  - actionable hint in CLI
- Errors are classified:
  - user error (bad flag, invalid query)
  - provider error (auth, rate limit)
  - tool error (permission denied, timeout)
  - internal error (bug)

---

## 13. Performance plan

- Startup:
  - lazy-init heavy subsystems (search DB open, git status cache) unless needed.
- Search:
  - precomputed lowercased path store
  - partial sort for pagination
  - parallel matching only above thresholds
- Grep:
  - mmap files where possible
  - file-based pagination + early termination
  - time budgets to keep UI responsive

---

## 14. Test strategy

See `docs/test-suite.md` (generated in this deliverable) and the `tests/` scaffold.

Key principles:
- Deterministic fixtures for RPC/session/search.
- Property tests for scoring invariants.
- Benchmarks for search latency and tool overhead.
- Extension conformance harness with golden outputs.

