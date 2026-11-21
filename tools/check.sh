#!/usr/bin/env bash
# Pre-push check (run by .githooks/pre-push, or by hand): the same checks as CI's "check" job.
set -euo pipefail
cd "$(dirname "$0")/.."
[ -d ui/node_modules ] || npm --prefix ui ci --silent
exec ./scripts/ci-local.sh
