#!/usr/bin/env bash
# Enforces the architecture boundaries from plan.md:
# - the rules engine and the applicant model are pure: no network, database,
#   keychain, async runtime or Tauri dependency, and no file/network/process
#   access in their source;
# - the rules watcher (when it exists) never depends on client-data crates.
set -euo pipefail
cd "$(dirname "$0")/.."
fail=0

deps() { cargo tree -p "$1" -e normal --prefix none --format '{p}' | awk '{print $1}' | sort -u; }

forbid() {
  local crate=$1; shift
  local tree; tree=$(deps "$crate")
  for f in "$@"; do
    if grep -qx "$f" <<<"$tree"; then echo "::error::$crate must not depend on $f"; fail=1; fi
  done
}

IO=(reqwest hyper ureq tokio async-std rusqlite libsqlite3-sys keyring tauri)
forbid applicant "${IO[@]}"
forbid rules-engine "${IO[@]}"
forbid grid-parser "${IO[@]}"

if cargo metadata --no-deps --format-version 1 | grep -q '"name":"rules-watcher"'; then
  forbid rules-watcher vault deid
fi

if grep -rnE 'std::(fs|net|process)|std::env::var|File::open' crates/applicant/src crates/rules-engine/src; then
  echo "::error::applicant and rules-engine must not do file, network, process or environment access"
  fail=1
fi

[ "$fail" = 0 ] && echo "dependency boundaries OK"
exit "$fail"
