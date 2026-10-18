#!/usr/bin/env bash
# Bumps the app version everywhere it appears. Usage: bump-version.sh patch|minor|major
# Source of truth: [workspace.package] version in Cargo.toml (Tauri reads it
# through the app crate; tauri.conf.json has no version of its own).
set -euo pipefail
cd "$(dirname "$0")/.."
part=${1:-patch}

current=$(sed -n '/^\[workspace.package\]/,/^\[/{s/^version = "\(.*\)"/\1/p;}' Cargo.toml | head -1)
IFS=. read -r major minor patch <<<"$current"
case "$part" in
  major) major=$((major + 1)); minor=0; patch=0 ;;
  minor) minor=$((minor + 1)); patch=0 ;;
  patch) patch=$((patch + 1)) ;;
  *) echo "usage: $0 patch|minor|major" >&2; exit 2 ;;
esac
next="$major.$minor.$patch"

# Cargo.toml: only the [workspace.package] version line.
awk -v v="$next" '
  /^\[workspace.package\]/ { inws = 1 }
  /^\[/ && !/^\[workspace.package\]/ { inws = 0 }
  inws && /^version = / && !done { print "version = \"" v "\""; done = 1; next }
  { print }
' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml

# ui/package.json and its lockfile.
node -e '
  const fs = require("fs");
  for (const f of ["ui/package.json", "ui/package-lock.json"]) {
    const j = JSON.parse(fs.readFileSync(f, "utf8"));
    j.version = process.argv[1];
    if (j.packages && j.packages[""]) j.packages[""].version = process.argv[1];
    fs.writeFileSync(f, JSON.stringify(j, null, 2) + "\n");
  }
' "$next"

# Cargo.lock entries for workspace crates.
cargo update --workspace --offline -q 2>/dev/null || cargo update --workspace -q

echo "$current -> $next"
