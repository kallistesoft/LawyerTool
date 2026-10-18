#!/usr/bin/env bash
# Runs the same checks as CI's "check" job. Commit only if this passes.
set -euo pipefail
cd "$(dirname "$0")/.."
npm --prefix ui run build >/dev/null
npm --prefix ui run check >/dev/null
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -q -- -D warnings
cargo test --workspace --locked -q 2>&1 | grep -E "^test result|FAILED|panicked" | awk '/FAILED|panicked/ {bad=1} {print} END {exit bad}' | tail -1
./scripts/check-deps.sh
echo "ci-local: all checks passed"
