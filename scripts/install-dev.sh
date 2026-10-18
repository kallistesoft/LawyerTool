#!/usr/bin/env bash
# Builds "LawyerTool Dev" (debug build: automatic test vault, no passphrase)
# and installs it in ~/Applications, replacing the previous copy.
#   scripts/install-dev.sh               build + install (foreground)
#   scripts/install-dev.sh --background  same, detached, log in target/install-dev.log
# The git hooks in .githooks/ run the background form after each commit,
# merge and pull (enable once: git config core.hooksPath .githooks).
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT=$(pwd)
LOG="$ROOT/target/install-dev.log"

if [ "${1:-}" = "--background" ]; then
  mkdir -p target
  nohup "$0" > "$LOG" 2>&1 &
  echo "LawyerTool Dev: rebuilding in the background (log: target/install-dev.log)"
  exit 0
fi

[ "$(uname)" = "Darwin" ] || { echo "install-dev.sh: macOS only"; exit 1; }

# One build at a time: a newer request waits for the running one.
LOCK="$ROOT/target/install-dev.lock"
mkdir -p target
until mkdir "$LOCK" 2>/dev/null; do sleep 2; done
trap 'rmdir "$LOCK"' EXIT

# Toolchains (hooks run with a minimal environment).
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
command -v fnm >/dev/null && eval "$(fnm env --shell bash)"

echo "== $(date '+%F %T') building LawyerTool Dev at $(git rev-parse --short HEAD)"
[ -d ui/node_modules ] || npm ci --prefix ui --ignore-scripts --no-fund
node ui/node_modules/@tauri-apps/cli/tauri.js build --debug --bundles app --config src-tauri/tauri.dev.conf.json

APP="LawyerTool Dev.app"
SRC="$ROOT/target/debug/bundle/macos/$APP"
DEST="$HOME/Applications/$APP"
mkdir -p "$HOME/Applications"

running=0
if pgrep -f "$DEST/Contents/MacOS/" >/dev/null; then
  running=1
  osascript -e 'tell application "LawyerTool Dev" to quit' >/dev/null 2>&1 || true
  for _ in $(seq 1 20); do pgrep -f "$DEST/Contents/MacOS/" >/dev/null || break; sleep 0.5; done
  pkill -f "$DEST/Contents/MacOS/" 2>/dev/null || true
fi

rm -rf "$DEST"
ditto "$SRC" "$DEST"
echo "== installed $DEST ($(git rev-parse --short HEAD), $(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1))"

[ "$running" = 1 ] && open "$DEST" && echo "== relaunched"
exit 0
