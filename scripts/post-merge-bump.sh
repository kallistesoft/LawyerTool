#!/usr/bin/env bash
# Local replacement for .github/workflows/release-version.yml while GitHub
# Actions are disabled: after a PR is merged into main, bump the version,
# commit "chore(release): vX.Y.Z", tag and push.
#   scripts/post-merge-bump.sh [patch|minor|major]   (default: patch)
set -euo pipefail
cd "$(dirname "$0")/.."
part=${1:-patch}
export LAWYERTOOL_NO_DEV_INSTALL=1   # the final pull below rebuilds the dev app once
git checkout -q main
git pull -q --ff-only origin main
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
command -v fnm >/dev/null && eval "$(fnm env --shell bash)"
out=$(./scripts/bump-version.sh "$part")
version=${out##* }
git commit -qam "chore(release): v$version [skip ci]"
git tag -a "v$version" -m "LawyerTool v$version"
git push -q origin main --follow-tags
echo "released v$version"
unset LAWYERTOOL_NO_DEV_INSTALL
./scripts/install-dev.sh --background >/dev/null 2>&1 || true
