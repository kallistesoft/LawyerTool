# Decisions

Non-blocking choices Claude made with its default; owner validates later. When a choice isn't blocking, write it
here with the default taken and keep working; ask only when it blocks.

| Date | Decision | Default taken | Status |
|---|---|---|---|
| 2026-11-21 | Pre-push check in this repo | `.githooks/pre-push` (this repo sets `core.hooksPath=.githooks`, which hides the global hook) hands over to `~/.config/git/hooks/pre-push`, i.e. `tools/check.sh` → `scripts/ci-local.sh` | to validate |
