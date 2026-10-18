# LawyerTool threat model

Status: v2, 2026-10-18 (after M3 hardening). To be reviewed with the lawyer and by
the independent security reviewer (see docs/security-review.md).

## 1. What we protect

| Asset | Where it lives | Why it matters |
|---|---|---|
| Client form answers and identity (names, contacts, ID numbers, history) | Encrypted vault (`lawyertool.db`) | Solicitor–client privilege; law society duties; PIPEDA; Quebec Law 25 |
| Draft and approved notes | Encrypted vault | Same as above; legal advice |
| Database key (DEK) | Memory while unlocked; wrapped in the OS keychain and in `lawyertool.keys` | Decrypts everything |
| Passphrase | The lawyer's memory only | Unwraps the keychain copy of the DEK |
| Recovery phrase (24 words) | Paper, firm safe | Unwraps the key-file copy of the DEK (new laptop, lost keychain) |
| Anthropic API key | Backend only (M2; OS keychain) | Spend, data sent under the firm's account |
| Approved rules snapshots | Encrypted vault / app bundle | A wrong rule produces a wrong eligibility opinion |
| Audit log | Encrypted vault | Proves what left the machine, when, where |
| Update signing key | Offline + CI secret | Whoever holds it can ship code to every install |

## 2. Assumptions

- The laptop has **full-disk encryption** (FileVault / BitLocker) and a screen
  lock. The app cannot protect an unencrypted, unlocked, stolen laptop.
- The OS account is the lawyer's own; administrators are trusted.
- Official sites (canada.ca, ircc.canada.ca) are trusted for *content
  authority* but their pages are treated as *untrusted input* for parsing.
- Claude is used under a commercial agreement with zero data retention (to
  confirm, see §6).

## 3. Trust boundaries

```
[client file] --picked--> [Rust backend] <--IPC--> [WebView UI]
                              |   \
                              |    `--> [vault: SQLCipher + keychain]
                              |
                     [net: allowlist + audit] --HTTPS--> canada.ca, ircc.canada.ca,
                                                         api.anthropic.com, update host
```

- The **WebView** is untrusted relative to the backend: strict CSP
  (`default-src 'self'`, no remote connect), no global Tauri object, only our
  own commands allowed by capability, no fs/shell/http plugins.
- **Two pipelines never mix** (brief rule 4): the rules watcher never reads
  client data (enforced by crate dependencies, `scripts/check-deps.sh`);
  client analysis never fetches the web.

## 4. Threats and mitigations

### T1. Lost or stolen laptop
- Vault is SQLCipher (AES-256) with a random 256-bit key; no plaintext SQLite
  header; tested by `vault::nothing_readable_at_rest`.
- DEK is wrapped by an Argon2id key (64 MiB, 3 passes) from the passphrase,
  stored in the OS keychain; the passphrase is never stored.
- Copying `lawyertool.db` + `lawyertool.keys` to another machine is useless
  without the 24-word recovery phrase (tested: `backup_restores_on_a_new_computer`).
- Auto-lock after 5 min idle: the DB is closed and the key wiped from memory
  (`Zeroize`). SQLCipher's `cipher_memory_security` (page locking) is off: on
  Windows it recursed until stack overflow when `VirtualLock` hit its quota;
  swapped memory is covered by the required full-disk encryption.
- The vault also locks when the computer wakes from sleep (wall-clock jump
  between idle-watch ticks).
- **Residual:** if the laptop is stolen *unlocked and awake*, data is
  exposed until auto-lock (5 min). Locking on the OS screen lock itself is
  not wired (needs platform hooks): follow-up.

### T2. Malware on the lawyer's computer
- Out of scope to defeat fully: malware running as the user can read the
  unlocked app's memory, log keystrokes and use the keychain.
- Reduce exposure: short unlock windows (auto-lock), no plaintext temp files,
  no client data in logs (lint: no print/log macros in `net`, `vault`, `audit`).
- **Residual:** high. Rely on OS protections and firm IT (EDR, updates).

### T3. Shared computer / other user account
- Keychain entries are per OS user; the vault files sit in the user's app
  data directory.
- **Recommendation:** one OS account per lawyer; never share the passphrase.

### T4. Malicious client file (M2)
- Files are read only through the file picker; the app never scans folders.
- Parsers are pure Rust (no macros, no external viewers, no OCR in MVP);
  size limits; parse errors are refused, not guessed.
- File content is never executed or rendered as HTML in the WebView.
- Input limits on everything the app stores (identity fields, profile size,
  list lengths).
- The form importer does not exist yet (waiting for a sample export); its
  hostile-file tests come with it.

### T5. Prompt injection (website text or client answers sent to Claude)
- Claude calls get **no tools**; outputs must match a schema and are
  validated by code; grid numbers never come from Claude alone; every value
  quoted from a page must appear verbatim in it.
- Rules watcher: parser is deterministic; Claude only proposes (prose pages,
  parser fallback, diff explanations); the lawyer approves.
- Client analysis: the eligibility math is deterministic Rust; Claude drafts
  text only; the lawyer edits and approves before export.
- Tested: injected "ignore previous instructions" text does not change parsed
  values (`grid-parser` tests); all page parsers are fuzzed with corrupted
  copies of the real pages and random markup (never panic, bounded time);
  lookalike digits and invisible characters fail loudly.
- De-identification matches on a normalized skeleton (invisible characters
  removed, full-width and Cyrillic/Greek lookalikes folded), catches
  spaced-out names and spelled-out emails; a property test checks no part of
  the client's name survives any of these disguises.

### T6. Network attacker / hostile redirect
- `net` is the only HTTP client: HTTPS only (rustls, OS trust store), exact
  host allowlist, no IP literals, credentials or custom ports, every redirect
  re-checked (max 3), 5 MB cap, timeouts.
- Every attempt is audited (metadata only), blocked ones included; a call
  fails if its audit entry cannot be written (so nothing leaves while locked).

### T7. Compromised or changed official page (rules watcher, M2b)
- Implemented: 13 fixed URLs, content-type allowlist per source, hash of the
  normalized `<main>` content; changes produce a value-by-value diff with
  the official row text, never an automatic update; the lawyer approves,
  and approved snapshots take effect from the approval date only.
- Cross-checks: summary vs detail tables, page vs page, golden profiles and
  the official calculator oracle must still agree before approval.
- Last approved snapshot stays in force on any failure; stale warnings.

### T8. Data sent to Claude
- De-identification before sending (names, contacts, IDs stripped), preview
  of exactly what is sent, identifiers re-attached locally (M2).
- Audit log records category, host, time and source file name for every call.
- Per-user API keys with spend limits; zero data retention (to confirm).

### T9. Malicious or broken update (M1 step 7)
- Signed updates (`tauri-plugin-updater`, minisign); the private key stays
  offline / in CI secrets; only the public key ships in the app.
- Update host on the allowlist; HTTPS; user is prompted, never force-restarted;
  `min_version` for critical fixes.
- Dormant until configured (last step before release); the `zip` feature
  (v1 zipped updates, 3 advisories) is not compiled in.
- **Open:** update host location; signing key generation (by the owner).

### T10. Supply chain (dependencies, CI)
- Exact-pinned versions for sensitive crates and frontend deps; lockfiles
  committed; `cargo audit` and `npm audit` in CI; GitHub Actions pinned to
  commit SHAs; minimal frontend (Svelte, no runtime framework deps).
- Dev-only tools (the jsdom oracle, jQuery 2.2.4) are never shipped.
- `cargo audit` (2026-10-18): 0 vulnerabilities. Accepted warnings, Linux /
  GTK only (not built or shipped): glib RUSTSEC-2024-0429 (unsound),
  proc-macro-error RUSTSEC-2024-0370 (unmaintained). The shipped UI has a
  single runtime npm dependency (`@tauri-apps/api`), 0 findings.

### T11. Wrong legal output (integrity, not confidentiality)
- Deterministic engine, cited to the official tables; 29 golden profiles
  recorded on the live IRCC calculator; differential fuzzing against the
  calculator's own script; every divergence between official sources is
  ruled by the owner and flagged for mandatory lawyer review.
- Notes are drafts until the lawyer approves them.

### T12. Lost recovery phrase and lost keychain
- Both lost = data unrecoverable, by design. The setup screen requires the
  user to confirm two of the 24 words; encrypted backups carry the key file.
- **Recommendation:** store the phrase in the firm safe; test a restore once.

## 5. Logging and temporary files

- No client data, keys, passphrases or request/response bodies in logs,
  crash reports or temp files (brief rule 9).
- SQLCipher memory locking is disabled (see T1), so it no longer prints
  `VirtualLock` warnings on Windows.
- SQLite temporary files are kept in memory (`temp_store = MEMORY`).
- Exports (PDF / Word) are written only where the user chooses in the
  native dialog, after mandatory review, and are marked as unencrypted.

## 6. Open items

- [x] Lock on OS sleep (wall-clock jump). [ ] Lock on OS screen lock (platform hooks).
- [ ] Update host and signing key (M1 step 7).
- [ ] Apple notarization and Windows EV code signing.
- [ ] Anthropic commercial terms and zero data retention confirmation.
- [ ] Law society guidance and client consent wording.
- [x] Hostile page / feed fuzzing, injection and de-identification
      adversarial tests, `cargo audit` triage (M3).
- [ ] Hostile client-file tests (with the importer).
- [ ] Independent security review (docs/security-review.md).
