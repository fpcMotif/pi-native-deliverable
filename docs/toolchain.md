# Toolchain and commit hooks

## Git commit pre-check workflow (strict production policy)

This repo now uses a `prek`-compatible pre-commit configuration in
`.pre-commit-config.yaml`.

Suggested local onboarding:

1. Install `prek` (or your preferred distribution method from the `prek` docs).
2. Run from repo root:
   - `prek install`
3. Optional: trigger a full pass before pushing:
   - `prek run --all-files`

The configured hooks currently enforce:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-features`
- `cargo clippy --workspace --all-features -- -D warnings`
- `cargo test --test tool_sandbox --test extension_policy --test session_path_guards`

### Policy hardening rules

These checks are intentionally stricter than default style checks:

- tool sandbox write/read/edit gates are exercised on committed code
- dangerous shell commands are blocked by `pi_tools::is_dangerous_command`
- session path resolution is validated to stay within workspace (`SessionStore::resolve_session_path`)
- denylist paths (`.env`, `.env.local`, `.bash_history`, `id_rsa`, `id_rsa.pub`) are enforced

## CI behavior

GitHub Actions runs `cargo fmt`, `cargo clippy`, and `cargo test` directly for reliability.
The workflow also runs the hook configuration through `prek` when `prek` is installed in the CI
environment.

## Memory rules for your future CLI workflow

- Always use: `prek install` once, then `pre-commit` checks run automatically.
- Before every push, run:
  1. `prek run --all-files`
  2. `cargo test --workspace`
- After policy updates, update both:
  - `.pre-commit-config.yaml`
  - `.github/workflows/ci.yml`

## CLI print mode contract

`pi --print --prompt "..."` (or `pi -p --prompt "..."`) is equivalent to `--mode print`.

Stream behavior and exit codes in print mode:

- `stdout`: assistant text only when the request succeeds.
- `stderr`: parse/runtime/tool/provider errors only.
- Exit codes:
  - `0`: success.
  - `2`: invalid input (for example, missing `--prompt`).
  - `10`: runtime failure while executing agent logic.
  - `20`: provider failure surfaced by the runtime.
  - `21`: tool execution failure surfaced by the runtime.
