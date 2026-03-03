#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="${1:-$(git rev-parse --show-toplevel)}"
cd "$ROOT_DIR"

echo "[pre-commit] strict policy checks for pi-native-rs"

echo "[pre-commit] formatting"
cargo fmt --all -- --check

echo "[pre-commit] compile"
cargo check --workspace --all-features

echo "[pre-commit] lint"
cargo clippy --workspace --all-features -- -D warnings

echo "[pre-commit] policy gates"
cargo test --test tool_sandbox --test extension_policy --test session_path_guards
echo "[pre-commit] done"
