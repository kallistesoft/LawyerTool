# Brief for the independent security review

LawyerTool is a local-first desktop app (Tauri v2: Rust backend, Svelte
WebView) that stores privileged immigration client files and computes
Express Entry eligibility. Please review it against `docs/threat-model.md`.

## Scope

1. **Data at rest** — `crates/vault`: SQLCipher, key wrapping (Argon2id
   passphrase slot in the OS keychain, recovery slot from a 24-word phrase),
   migrations with encrypted backups, auto-lock, zeroization.
2. **WebView boundary** — `src-tauri/capabilities/default.json`,
   `src-tauri/tauri.conf.json` (CSP), every command in `docs/ipc-surface.md`.
   Frozen by `src-tauri/tests/security_config.rs`.
3. **Network** — `crates/net` (only HTTP client: HTTPS, exact allowlist,
   redirects, size cap, content types, audit), `crates/audit`, the updater
   wrapper in `src-tauri/src/updates.rs` (dormant until configured).
4. **Untrusted input** — official pages (`crates/grid-parser`,
   `crates/rules-watcher`), the draws feed, profile JSON from the WebView.
5. **De-identification** — `crates/deid` (what would be sent to Claude;
   no call is made yet: the Claude integration is deferred).
6. **Exports** — `crates/report`, `src-tauri/src/export.rs` (review gate,
   native dialog, unencrypted file warning).
7. **Supply chain and build** — `Cargo.lock`, `ui/package-lock.json`,
   `.github/workflows/*` (SHA-pinned actions), `scripts/check-deps.sh`.

Out of scope for now: the updater host and code signing (not configured),
the Claude API integration (not built), the form importer (not built).

## Known and accepted

- SQLCipher `cipher_memory_security` is off (it recursed into a stack
  overflow on Windows when `VirtualLock` hit its quota); swap protection
  relies on the required full-disk encryption.
- No lock on OS screen lock yet (idle 5 min and system sleep only).
- Linux/GTK-only advisories (glib, proc-macro-error): Linux is not shipped.
- Dev builds open an automatic test vault (no passphrase); compiled out of
  release builds (`cfg(debug_assertions)`, checked by `check-deps.sh`).

## How to build and test

```sh
npm ci --prefix ui && npm ci --prefix tools/oracle
./scripts/ci-local.sh                                   # fmt, clippy, tests, boundaries
cargo test -p oracle-tests -- --include-ignored         # differential fuzz vs IRCC calculator
PROPTEST_CASES=600 cargo test -p grid-parser --test fuzz
cargo test -p rules-watcher -- --include-ignored        # live fetch of the 13 IRCC pages
cargo audit && npm audit --prefix ui
node ui/node_modules/@tauri-apps/cli/tauri.js build --bundles app   # release build (real vault)
```

Suggested focus: the IPC commands as an attacker controlling the WebView
(e.g. a compromised dependency in the UI bundle), the key-wrapping and
recovery design, and whether any client data can reach the network, logs or
temp files.
