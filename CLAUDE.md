# Project brief: secure desktop app for immigration eligibility screening

Read this fully before doing anything. It records decisions already made in a planning conversation. Do not re-litigate them; ask if something is unclear or conflicts with reality.

## What we're building
A standalone desktop app (macOS + Windows) for a Canadian immigration lawyer / law firm. It screens client eligibility: it takes a client's form answers, calculates eligibility deterministically from official rules, and uses Claude to draft an explanatory note for lawyer review. Privileged client data, so security comes first.

## Revision 2026-10-21: web app instead of a desktop app
The user dropped the Mac/Windows app (no notarization, signing or updater) now that Kallistesoft has its own hosting. What changed, ruled by the user:
- **Static web app at `audit.kallistesoft.ca`** (GoDaddy cPanel, deployed by `.github/workflows/web.yml`). No backend, no database, no PHP. One lawyer for the pilot; Chrome or Edge (File System Access API).
- **Client data stays on the lawyer's computer.** The Rust engine (rules engine, de-identification, reports, vault) is compiled to WebAssembly (`crates/web`) and runs in the browser, in a Web Worker. The vault is one encrypted file on the lawyer's disk (`crates/vault-file`: Argon2id passphrase slot + 24-word recovery slot, AES-256-GCM body), read and written in place. The server never receives client data.
- **Same behaviour as the desktop app:** Paramètres keeps the input folder (form results) and the output folder (reports, never overwritten), chosen with the browser's folder picker and remembered by the browser; auto-lock after 5 minutes or after sleep.
- **Network:** the page only fetches its own origin. The strict CSP in `ui/public/.htaccess` (`connect-src 'self'`) blocks any other destination.
- **Rules watcher:** a daily CI job runs the native watcher (`watch-publish`, feature `collect`) and publishes `watch/latest.json` (public data) with the site. The browser compares it with the vault; the lawyer still approves every change.
- **Tech stack kept:** Rust engine + Svelte UI. A full TypeScript rewrite and a PHP server app were rejected.
- The Tauri shell (`src-tauri`, `crates/vault`, `crates/net` in the app, updater) stays until the web pilot works, then gets deleted. The "Decisions (final)" below are superseded where they mention the desktop, the keychain, installers or updates.

## Decisions (final)
- **Stack:** Tauri (Rust backend + web frontend). Not Electron, not Compose Multiplatform.
- **Standalone:** one double-click installer. No Python/Node/Java required on user machines. Stay in Rust for the backend (parse PDFs/Word/CSV with crates). No OCR in the MVP; if added later it must be a bundled sidecar.
- **Local-first:** no cloud backend, no telemetry, no analytics.
- **Storage:** SQLite with SQLCipher (AES-256). Key derived from passphrase with Argon2id, held in the OS keychain (macOS Keychain / Windows Credential Manager). Auto-lock on inactivity and wipe keys from memory on lock.
- **All network access happens in the Rust backend only.** The frontend never touches the network or the API key. Strict CSP, minimal Tauri command allowlist and file-path scopes.
- **Outbound destinations (the only ones):** the Claude API, an allowlist of official immigration sites (canada.ca / IRCC and others the user names), and the update host. All over HTTPS.
- **Files:** the app reads only files the user picks via a file dialog, or (revised 2026-10-18) the form-results *input folder* the user chose in Paramètres: read on demand only (when the user clicks "Analyser"), one level deep, `.xlsx`/`.csv` only, files never modified. Never scan any other folder. Reviewed reports go to the *output folder* the user chose (never overwritten), or a save dialog if none is set.
- **Updates:** `tauri-plugin-updater` with signed bundles and a `latest.json` on a private static host. Prompt the user rather than forcing restarts, support a minimum-version field for critical fixes, and keep a beta channel. Keep the updater private key offline/in CI secrets. Version the DB schema and take an automatic encrypted backup before migrating.
- **Distribution:** Apple Developer account + notarization; Windows code-signing cert (EV preferred). CI via GitHub Actions with `tauri-action` building .dmg and .msi.

## Core design rules
1. **Eligibility math is deterministic Rust code**, unit-tested against cases the lawyer has verified. Claude does NOT calculate scores.
2. **Claude's role:** extract/normalize messy form answers, apply the rules text for explanation, draft the note. Output must be structured and validated.
3. **De-identify before sending to Claude.** Strip names, contact details, ID numbers and other identifiers locally; show a preview of exactly what will be sent; re-attach identifiers locally afterward.
4. **Two separate pipelines that never mix:**
   - *Rules watcher* (fetch from allowlisted official sites, Claude extracts thresholds/point grids/criteria into a structured rules file). It never sees client data.
   - *Eligibility analysis* (sees client data, has no web access, uses only an approved rules snapshot).
5. **Rules snapshots are lawyer-approved.** A "Check for rule changes" action shows a diff (each value with source URL and fetch date); only approved snapshots feed the calculator. Every note records the snapshot version/date/sources. Warn prominently when the snapshot is stale. If extraction fails, keep the last approved snapshot; never guess. Add sanity checks that flag unusual value jumps.
6. **Treat websites and client files as untrusted (prompt injection).** Claude calls on that content get no tools/actions; accept only validated structured output; use pure-text PDF parsing.
7. **Lawyer review is mandatory.** The note is a draft shown with source answers and rules cited; the lawyer edits and approves before saving/exporting (PDF or Word).
8. **Audit log** of every outbound call (what category of data, when, which destination, which file) WITHOUT the content itself.
9. **No sensitive data in logs, crash reports or temp files.** Encrypted backups/exports only.
10. **API key hygiene:** per-user keys with spend limits rather than one shared key. Check Anthropic commercial terms; pursue zero data retention.

## MVP scope
In: passphrase unlock, auto-lock, encrypted DB; import client form answers via file picker (likely a Microsoft Forms Excel/CSV export, format TBD); de-identification with preview; deterministic eligibility/score engine; Claude extraction + note drafting; rules watcher with diff/approval flow; review screen + export; audit log; signed installers + auto-updater.

Out (later): inbox listening, OCR for scans, multi-user features, any cloud component.

## Plan / phases (part-time estimate ~3.5-4 months)
- **Phase 0, decisions (~1 wk):** one-page threat model (lost laptop, malware, shared computer, malicious client file, prompt injection); API terms and ZDR; law society guidance and client consent wording; pick first program(s) to screen and their rules.
- **Phase 1, skeleton + security (~2 wks):** Tauri project, CI builds, locked-down config, encrypted DB, keychain, auto-lock, signing accounts started early, update pipeline.
- **Phase 2, eligibility flow (~3-4 wks):** import, de-identification, rules engine, Claude step, review and export, audit log.
- **Phase 2b, rules watcher (~2-3 wks):** allowlisted fetch module, extraction to structured rules file, diff and approval, stale warnings.
- **Phase 3, hardening (~2 wks):** hostile-PDF/web-page/prompt-injection tests, `cargo audit` + `npm audit`, minimal frontend deps, independent security review.
- **Phase 4, pilot (~2-3 wks):** compare drafts against the lawyer's manual analysis on low-risk real cases; fix; release.

## Compliance context
Users are Canadian lawyers: law society duties to safeguard client information, PIPEDA, Quebec Law 25. Device-level risk: require full-disk encryption (FileVault/BitLocker); the app can't protect an unencrypted stolen laptop.

## Open questions (ask the user, don't assume)
- Which immigration program(s) does v1 screen for? (Determines which official pages feed the rules.)
- Exact format of the client form export (ask for a sample with fake data).
- Who owns the Anthropic API key (firm vs each lawyer)?
- Where will the private update host live?
- Preferred frontend framework (keep dependencies minimal).

## How to work with me
- The user codes regularly. Be direct and technical.
- Start with Phase 1 unless told otherwise. Propose the repo structure before generating lots of code.
- Prefer small, reviewable steps; add tests for the rules engine early.
