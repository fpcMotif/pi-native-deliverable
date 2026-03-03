# pi-native-rs Test Suite

This test suite is designed to make the rewrite **safe, fast, and regression-resistant** from day one. It is organized as:

- **Unit tests** (crate-local): correctness of pure logic and small components.
- **Integration tests** (`/tests`): black-box behaviors across crates and the `pi` binary.
- **Conformance tests** (`/tests/ext_conformance`): extension API compatibility and policy gating.
- **Property tests** (`proptest`): invariants for scoring, parsing, tree operations.
- **Benchmarks** (`criterion`): latency/throughput budgets (search, grep, tool overhead).
- **Fuzzing** (optional CI job): robustness of streaming JSON parsers and RPC framing.

---

## 1. Test coverage map

### 1.1 pi-protocol

**Unit**
- `rpc_message_roundtrip`: serde roundtrip for every message type.
- `rpc_version_rules`: version negotiation accepts patch/minor additions.
- `rpc_schema_validation`: jsonschema for externally-visible payloads.

**Property**
- `rpc_line_framing`: random JSON objects serialized + newline; parser splits correctly.

**Fuzz**
- `rpc_input_fuzz`: random bytes into framing decoder must not panic; must either parse or return error.

### 1.2 pi-config

**Unit**
- `merge_precedence`: global < project < CLI < env overrides.
- `paths_expand_tilde`: `~` expansion and platform path normalization.
- `invalid_settings`: produce structured diagnostics.

**Integration**
- `config_show`: `pi config --show` prints resolved config.
- `config_json`: `pi config --json` emits machine-readable report with provenance.

### 1.3 pi-core (agent + sessions)

**Unit**
- `message_queueing`: `prompt` while streaming requires explicit queue behavior; `steer` interrupts; `follow_up` waits.
- `abort_semantics`: abort halts model stream and tool chain after boundary.
- `session_tree_ops`: fork/navigate invariants.

**Property**
- `session_tree_is_acyclic`: random sequences of fork/navigate never produce cycles.
- `session_append_only`: entries are append-only and ids are unique.

**Integration**
- `session_roundtrip`: write JSONL, reload, and compare normalized structures.
- `compaction_preserves_contract`: after compaction, tool results referenced remain accessible (via sidecar or snapshot).

### 1.4 pi-llm (providers + streaming)

**Unit**
- `partial_json_tool_args`: incremental JSON fragments reconstruct to final args.
- `tool_args_schema_validation`: invalid args -> proper error events.
- `usage_accounting`: token/cost accounting aggregates per session.

**Integration**
- `openai_compatible_smoke`: mock server responds like OpenAI; agent streams + tools.

**Fuzz**
- `provider_event_fuzz`: random event sequences should not crash state machine.

### 1.5 pi-tools (built-ins + sandbox)

**Unit**
- `read_limits`: max file size; binary detection.
- `write_path_policy`: denies writes to restricted patterns.
- `edit_patch`: multiple replacements; idempotency on no-op.
- `bash_timeout`: command killed after timeout; no zombie.

**Integration**
- `tool_audit_log`: each tool call produces an audit record with allow/deny and rule id.

### 1.6 pi-search (file index + fuzzy + grep)

**Unit**
- `query_parser`: constraints parsing and location suffix parsing.
- `fuzzy_scoring_ordering`: filename bonus outranks path-only in defined cases.
- `pagination`: stable paging for same snapshot.
- `binary_detection`: NUL heuristic behaves as expected.

**Property**
- `score_monotonicity`: exact match should never score below fuzzy match for same file.
- `stable_sort_tiebreakers`: for equal score, tiebreakers are deterministic.

**Integration**
- `index_updates_on_fs_events`: create/delete/rename in fixture repo updates index.
- `git_status_cache`: modified files get expected status and influence ranking.
- `grep_modes`: Plain/Regex/Fuzzy produce expected highlights and pagination.

**Benchmarks**
- `fuzzy_50k_files_p95`: build 50k synthetic paths and ensure p95 <= target.
- `grep_corpus_budget`: first page returned under time budget.

### 1.7 pi-ext (extensions)

**Unit**
- `policy_allow_deny`: capability checks return correct decision + reason.
- `manifest_validation`: missing fields or mismatched capability declarations fail validation.

**Integration**
- `wasm_extension_smoke`: load minimal wasm extension that registers tool; invoke tool.
- `js_compat_extension_smoke`: load JS extension (if enabled); verify hostcalls are gated.
- `hot_reload`: update extension file; `/reload` picks up changes without restart.

**Conformance**
- `legacy_extension_catalog`: run a known set of extensions through:
  - load time budget
  - registered tool parity
  - restricted hostcalls are denied under safe policy

---

## 2. Fixtures

`/fixtures` contains deterministic repositories:

- `fixtures/repo_small/`: ~30 files, mixed types, includes binary file.
- `fixtures/repo_git/`: initialized git repo with modified/staged/untracked files.
- `fixtures/corpus_grep/`: files crafted for grep edge cases:
  - huge single-line files
  - UTF-8 boundaries
  - regex pathological patterns

All fixtures are generated (or updated) via a single script to keep them consistent.

---

## 3. Suggested CI matrix

- `cargo test --workspace`
- `cargo test --workspace --features js-compat` (optional)
- `cargo test --workspace --features wasm-ext`
- `cargo test --workspace --features mimalloc` (optional)
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --all -- --check`
- `cargo bench -p pi-search` (nightly / scheduled)
- `cargo fuzz run rpc_input_fuzz` (nightly / scheduled)

---

## 4. Integration test scaffolding (examples)

See `/tests` for executable skeletons.

