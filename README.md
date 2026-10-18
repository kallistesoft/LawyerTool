# LawyerTool

Secure desktop app (macOS, Windows) for Canadian immigration eligibility
screening: deterministic Express Entry CRS and FSW / CEC / FST rules, with
Claude drafting the explanatory note for lawyer review. See `CLAUDE.md` (brief),
`plan.md` (milestones) and `docs/threat-model.md`.

## Development

Requirements on the dev machine only (users need nothing): Rust (rustup,
pinned by `rust-toolchain.toml`) and Node (pinned by `.node-version`, via fnm).

```sh
npm ci --prefix ui                     # once
node ui/node_modules/@tauri-apps/cli/tauri.js dev   # live: UI hot reload, Rust restart
./scripts/ci-local.sh                  # same checks as CI; run before committing
```

Debug builds open an **automatic test vault** (no passphrase, no auto-lock,
"MODE DEV" banner) in a separate data folder. Release builds always use the
real vault. To try the real passphrase flow in a debug build:
`LAWYERTOOL_REAL_VAULT=1 node ui/node_modules/@tauri-apps/cli/tauri.js dev`.

### Installed dev app, rebuilt after each change (macOS)

```sh
git config core.hooksPath .githooks    # once per clone
./scripts/install-dev.sh               # first install: ~/Applications/LawyerTool Dev.app
```

After that, every commit, merge, pull or branch switch rebuilds and reinstalls
**LawyerTool Dev** in the background (it is relaunched if it was open). Log:
`target/install-dev.log`. Skip once with `LAWYERTOOL_NO_DEV_INSTALL=1 git …`.
LawyerTool Dev has its own identifier (`ca.lawyertool.desktop.dev`) and data,
so it never touches a real installation.

## Releases and CI

**GitHub Actions are disabled** (to save the plan's minutes; re-enable with
`gh workflow enable ci.yml` etc.). Meanwhile:

- checks: `./scripts/ci-local.sh` before every commit (same as the CI
  "check" job; the Windows check only runs on GitHub);
- version: after each merge into `main`, `./scripts/post-merge-bump.sh`
  (`patch` by default, or `minor` / `major`) bumps, commits
  `chore(release): vX.Y.Z`, tags and pushes, like the disabled
  `release-version` workflow.

Installers: `release.yml` (disabled) or locally with
`node ui/node_modules/@tauri-apps/cli/tauri.js build`.
