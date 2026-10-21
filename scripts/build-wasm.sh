#!/usr/bin/env bash
# Builds the engine (crates/web) to WebAssembly for the web app, into
# ui/src/engine/ (generated; not in git). Needs the wasm32 target and
# wasm-bindgen-cli at the same version as the wasm-bindgen crate:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli --version 0.2.129 --locked
set -euo pipefail
cd "$(dirname "$0")/.."
profile=${1:-release}
flag=$([ "$profile" = release ] && echo --release || true)
cargo build -p web --target wasm32-unknown-unknown $flag
wasm-bindgen --target web --out-dir ui/src/engine --out-name engine \
  "target/wasm32-unknown-unknown/$profile/web.wasm"
