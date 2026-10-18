# IPC surface (WebView → Rust)

The WebView can call **only** these commands (capability
`src-tauri/capabilities/default.json`, frozen by
`src-tauri/tests/security_config.rs`) plus event listen/unlisten. No plugin
command (fs, shell, http, dialog, updater) is exposed to the WebView: the
dialog and updater plugins are driven from Rust only.

All commands that touch data require the vault to be unlocked
(`Session::with_vault`, error code `locked`) and count as activity for
auto-lock. Errors carry a stable code and a French message, never secrets
or client data.

| Command | Input | Checks | Returns |
|---|---|---|---|
| `app_info` | — | — | name, version |
| `vault_status` | — | — | exists, unlocked, idle minutes, dev flag |
| `vault_create` | passphrase | ≥ 12 chars; refuses if a vault exists; Argon2id off the main thread; passphrase in `Zeroizing` | recovery phrase (shown once, never stored) |
| `vault_unlock` | passphrase | wrong passphrase → `wrong_passphrase`; missing keychain entry → `keychain_missing` | — |
| `vault_recover` | 24 words, new passphrase | BIP-39 checksum; wrapped key bound to vault id | — |
| `vault_lock` | — | — | — (closes DB, wipes key, emits `vault-locked`) |
| `activity` | — | only when unlocked | — |
| `client_list` | — | unlocked | id, name, file number, updated |
| `client_get` | id | unlocked; profile must parse | identity + profile |
| `client_save` | id?, identity, profile | profile parsed strictly as `Applicant` (unknown fields refused); identity length limits (name 200, email 254, phone 40, file 64); profile ≤ 256 KiB; ≤ 100 jobs, ≤ 50 NOC rows; validation problems returned (draft saved) | id, problems |
| `client_delete` | id | unlocked | deleted? |
| `assess` | id, date? | ISO date; grid in force on the date (shipped + approved); engine refuses invalid profiles | CRS, eligibility, latest draws |
| `deid_preview` | id, date? | sends nothing | de-identified payload, size, redactions (no values) |
| `export_report` | id, date?, format, reviewed ids | **refused unless every ManualReview flag is in `reviewed`**; format pdf/docx only; path chosen in the native dialog opened by Rust (the WebView never supplies a path); file name from file number, not client name | saved path or null |
| `watch_status` | — | unlocked | runs, stale flag, pending changes, grids, draws |
| `watch_check_now` | — | one run at a time; fetches only the 13 fixed URLs through `net` (allowlist, audit) | summary |
| `watch_approve` | note? | something pending; candidate grid must have no issues after rulings | new snapshot ids |
| `watch_reject` | — | — | — |
| `update_check` | — | updater configured (off until the last step); endpoint host on allowlist; audited | update info |
| `update_install` | — | user-initiated; signature verified by the plugin; vault locked before restart; audited | (restarts) |

Events emitted to the WebView: `vault-locked`, `watch-updated` (no payload).

Network: every outbound request goes through `crates/net` (HTTPS only, exact
host allowlist `api.anthropic.com`, `www.canada.ca`, `ircc.canada.ca`,
redirects re-checked, 5 MB cap) and is written to the encrypted audit log;
the updater checks the same allowlist and audits its calls.
