# Toolchain and commit hooks

## Git commit pre-check workflow

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

## CI behavior

GitHub Actions runs `cargo fmt`, `cargo clippy`, and `cargo test` directly for reliability.
The workflow also runs the hook configuration through `prek` when `prek` is installed in the CI
environment.
