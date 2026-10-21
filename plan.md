# Plan: LawyerTool

## Context
`~/Desktop/LawyerTool` holds a standalone Tauri desktop app (macOS and Windows) for Canadian immigration lawyers. It screens client eligibility from form answers, computes the score deterministically from official rules, and has Claude draft an explanatory note for the lawyer to review. The brief is in `CLAUDE.md`; its decisions are final unless revised below.

This plan set the build order (the repo started with only the brief). Version 1 targets **Express Entry**: program eligibility plus the CRS score. The most important thing to prove early is that the engine's scores match the official IRCC calculator. That is why the engine and its test oracle come first, before the security plumbing. The engine handles no client data, so it doesn't need that plumbing yet.

## Glossary
- **Score (note SCG / CRS score):** the points total, 0–1200, computed by `rules-engine`.
- **Explanatory note:** the memo Claude drafts and the lawyer approves.
- **Grid snapshot:** a lawyer-approved, versioned, effective-dated rules data file. It contains:
  - the CRS points
  - the FSW 67-point grid
  - the program minimums
  - the language equivalency tables
- **Oracle:** a source of the official correct answer. There are two: the live IRCC calculator, used via Chrome, and its JavaScript file run in a headless test harness.

## Decisions (this conversation + brief)
- **Repo:** all work happens in `~/Desktop/LawyerTool` (remote `github.com/jefpoughon-tech/LawyerTool`), on branches like `m0/engine`. The first commit adds `CLAUDE.md` and this plan as `plan.md`.
- **Stack:**
  - Tauri v2 with a Rust backend.
  - Svelte + Vite frontend.
  - Toolchain: rustup + fnm, with `.node-version`.
  - Node is used for building and dev tests only; users' machines don't need it.
- **Standalone installer:** the `.dmg` and `.msi` work with no prior installs (details in M1).
- **v1 program:** Express Entry (FSW, CEC, FST eligibility, plus the CRS score).
- **Daily rules watch** is run by the app itself: after unlock if the last check was more than 24h ago, then every 24h while the app is open.
- **Scraping and number extraction are plain code, with no AI** (revises the brief's "Claude extracts thresholds"):
  - Fetching, table parsing and the draws JSON are deterministic Rust.
  - Claude is used only where scripts can't do the job:
    - (a) turning prose announcements into structured pending changes;
    - (b) proposing a new mapping when the parser breaks on a redesigned page;
    - (c) explaining a diff in plain language.
  - Claude's output is always a proposal, checked by code and approved by the lawyer. A grid number never comes from Claude alone.
  - The main AI value stays the client explanatory note (M2).

## Roadmap
| Milestone | Content | Status (2026-10-18, v0.1.15) |
|---|---|---|
| **M0** | Engine + oracle: Applicant model, CRS + eligibility + language conversion, grid parser, golden profiles, JS-oracle fuzzing | **Done.** 29 golden profiles match; fuzz 3 × 5,000, 0 unexplained mismatches; rulings D-001…D-004 recorded |
| **M1** | Security skeleton: Tauri lockdown, CI, vault + recovery key, auto-lock, net + audit, updater, signing | **Done**, except updater activation and signing (last step) and the clean-machine VM test |
| **M2** | Client flow: import, de-identification, Claude note, review + export | **Partly done.** De-identification + preview, review gate, PDF/Word export, input/output folders done. Waiting: form parser (needs a sample export). Deferred: Claude note |
| **M2b** | Daily watcher: fetch, parse, diff/approval, draws feed, Claude for prose/fallback | **Done** without the Claude parts |
| **M3** | Hardening (hostile inputs, injection, audits, independent review) | **Done**, except the independent review (brief ready: `docs/security-review.md`) |
| **M4** | Pilot against the lawyer's manual analyses | **Next** |

### Done outside the milestones
- **Settings screen (v0.1.13):** input folder for form results (read on demand, one level, `.xlsx`/`.csv`, never modified, SHA-256 tracking) and output folder for reviewed reports (never overwritten). Brief rule on files revised in `CLAUDE.md`.
- **French everywhere (v0.1.14):** every message, error and label shown in the app; a test fails if a score label has no French translation.
- **UI redesign (v0.1.15):** sidebar, light/dark design system, client list with score / program badges / review status (`client_overview`), assessment page with the key results first, form with section index and sticky save bar; dates in local time.
- **Vault fix (v0.1.12):** a migration backup used to empty the live key file; fixed, with a rebuild from the keychain and regression tests.

### Web app (2026-10-21, branch `web-port`)
The desktop app becomes a static web app at `audit.kallistesoft.ca`, with client data kept on the lawyer's computer (see `CLAUDE.md`, "Revision 2026-10-21"). Done: `crates/vault-file`, `crates/web` (wasm), the worker + File System Access layer in `ui/src/lib/api.ts`, the watcher publisher, `web.yml` and `.htaccess`. To do:
- user: create the `audit.kallistesoft.ca` subdomain in cPanel (docroot `public_html/audit.kallistesoft.ca`), add the repository secrets `FTP_SERVER`, `FTP_USERNAME`, `FTP_PASSWORD`, and set the pilot password in cPanel > Directory Privacy;
- try the real (non-dev) flows in Chrome: create the vault file, pick the folders, export, lock and unlock, recover;
- after the pilot: delete `src-tauri`, `crates/vault`, the updater and the release workflows; update `docs/threat-model.md` (no keychain: the passphrase alone protects a copied vault file; a compromised host could serve hostile code, mitigated by the CSP and CI-only deploys).

### Next steps
1. **2026-11-03:** IRCC switches to the grid with the labour-market factor. When its calculator is updated, re-record the golden profiles marked `oracle: pending` and check that the watcher picked up the change.
2. **Form import:** write the parser as soon as a sample export (fake data) arrives.
3. **M4 pilot:** compare with the lawyer's own analyses on low-risk real cases.
4. **Independent security review** with `docs/security-review.md`.
5. To decide: match imported forms to existing clients (email or file number) instead of always creating a new client.
6. ~~Last step: updater activation, Apple notarization, Windows signing~~: no longer needed (web app).

---

## M0: Engine + oracle (do first)
**Status 2026-10-18:** done. Golden profiles for the 2026-11-03 grid remain `oracle: pending` until IRCC updates its calculator.


### Layout (Cargo workspace; created now and used by every milestone)
```
Cargo.toml                    # workspace + shared lints
crates/applicant/             # canonical Applicant model (serde), shared by fixtures, import, engine
crates/rules-engine/          # pure: no I/O / network deps
  src/{crs.rs, fsw67.rs, eligibility.rs, language.rs, grid.rs}
  fixtures/crs/NN-*.json      # golden profiles
  fixtures/sources/           # saved official pages (HTML/JS/JSON) with fetch date + hash
crates/grid-parser/           # deterministic HTML-table → Grid parser (reused by the M2b watcher)
crates/deid, vault, net, audit, rules-watcher/   # stubs until M1/M2
tools/oracle/                 # Node dev-only harness: runs IRCC calculator JS in jsdom
src-tauri/, ui/               # created in M1
```
In CI, a `cargo tree` check fails the build if:
- `rules-engine` depends on I/O or network crates;
- `rules-watcher` depends on `vault` or `deid`.

### Rule change on 2026-11-03 (found 2026-10-18 in `modifications.html`)
IRCC changes the CRS grid on 2026-11-03, so M0 has to support two grid versions from the start:
- **`crs-2026-10-16`** (current) is what the calculator implements today. Job-offer points are 0, because the code for them is commented out in `scg-outil-udit.js`.
- **`crs-2026-11-03`** (pending) adds a new **labour market integration** factor, worth up to 140 points with a spouse or 150 without. It is the *greater* of two options:
  - **Canadian work:** points for how long the person worked (the current 70/80 table), plus high-wage occupation points for at least 12 months of qualifying experience: 70 for occupations whose median wage is at least 2× the national median, 40 at 1.5×, 20 at 1.3×.
  - **Qualifying job offer in a high-wage occupation:** 100 at ≥2×, 70 at 1.5× to under 2×, 50 at 1.3× to under 1.5×.
  - Skill transferability does not change. An LMIA is no longer required. High-wage occupations are listed by NOC code on `salaire-eleve.html`.
- **Impact on the Applicant model:** Canadian work and the job offer now carry a NOC code and a number of months, and the engine looks up the high-wage tier from the grid. Job offers still matter for FSW/FST eligibility (any TEER 0–3 job, not only high-wage ones).
- **Oracle:** golden profiles for `crs-2026-11-03` can be checked against the live calculator only once IRCC updates it, probably on 2026-11-03. Until then they are checked against the announcement tables, and marked `oracle: pending`.

### Steps (one reviewable commit each)
1. **Applicant model** (`crates/applicant`):
   - Fields mirror the calculator's questions (form markup saved in `fixtures/sources/`): marital status and spouse status, age, education, language tests, work experience, certificate, job offer, nomination, sibling, Canadian education, and the spouse's factors.
   - Language is stored as **raw test results** (test type + four skill scores), not CLB levels.
   - Includes the fields eligibility needs: continuous work in a NOC TEER level, settlement funds, and family size.
2. **Grid model + parser** (`grid.rs`, `crates/grid-parser`):
   - A `Grid` struct with `effective_from`, the CRS tables, the FSW 67 table, the program minimums, and the language equivalency tables.
   - The parser reads saved copies of these pages:
     - `criteries-scg.html`
     - the FSW criteria page
     - CEC and FST requirements
     - the language test equivalency charts
   - Exact URLs are pinned in `fixtures/sources/README.md`.
   - It anchors on table captions and row labels, records the CSS path and cell text of every value, and **fails loudly on any unexpected structure** rather than guessing.
   - v0 grid = the parser's output, reviewed by hand against the pages.
3. **Language conversion** (`language.rs`):
   - Converts IELTS, CELPIP, PTE Core, TEF Canada and TCF Canada results to CLB/NCLC, per skill, using grid tables.
   - Table-driven tests cover every band boundary.
4. **CRS engine** (`crs.rs`):
   - Computes core/human capital, spouse, skill transferability (capped at 100) and additional points (capped at 600, including the French bonus).
   - Returns the total and a per-factor breakdown, each item citing the grid row it used.
   - Uses the snapshot in force on the assessment date.
5. **Eligibility** (`fsw67.rs`, `eligibility.rs`):
   - FSW 67-point grid.
   - Minimums for FSW, CEC and FST: language, experience, funds, job offer/certificate for FST.
   - The result lists, per program, `eligible` / `not eligible (reasons)` / `needs lawyer input (missing data)`. It never guesses.
6. **Golden profiles** (`fixtures/crs/golden/`, 29 synthetic profiles; recorded 2026-10-18, all match):
   - Coverage:
     - marital status (single, spouse accompanying, Canadian/not-accompanying spouse scored as single);
     - age 17/18/20/29/30/35/44/45+;
     - all education levels, including two credentials;
     - CLB 4–10+, second language, French bonus at both thresholds;
     - Canadian work 0–5+, foreign work 0/1–2/3+;
     - certificate, job offer before and after 2026-11-03, nomination, sibling, Canadian education;
     - every cap, and the minimum profile.
   - `expected` holds the subtotals, the total and the eligibility result, recorded with the date, the calculator JS hash and the grid id.
   - **Recording uses Claude in Chrome** (`claude-in-chrome` skill, dev time only):
     - fill `verifier-note.html` → "Calculatrice du SCG" for each profile and read the breakdown;
     - review it, then commit.
   - Synthetic data only, because the page loads Adobe analytics.
7. **JS-oracle differential fuzzing** (`tools/oracle`, dev and CI only, never shipped):
   - Node + jsdom loads the saved calculator HTML and `scg-outil-udit.js`, both pinned by hash.
   - It sets form values, triggers the calculation and reads the result.
   - It runs in CI with no secrets and no network.
   - A `proptest` generator in Rust produces random valid Applicants, about 5,000 per CI run. Each is mapped to form values, and the engine's result is compared with the oracle's.
   - A mismatch prints a shrunk minimal counterexample, which gets added as a new golden profile.
   - Chrome remains the spot-check that the harness matches the live page.
   - **Mismatch triage** (a fuzz mismatch is never ignored, retried away or "fixed" by loosening the comparison):
     1. **Reproduce:** proptest shrinks the failing profile to a minimal counterexample. Its seed is saved in `proptest-regressions/`, which is committed, so CI re-runs it forever. The per-factor diff shows which factor differs.
     2. **Classify:** each mismatch falls into exactly one bucket.
        - **Invalid profile:** the generator built something impossible, e.g. 6 years of Canadian work at age 19, or spouse fields on a single applicant. Fix: tighten the generator *and* make `Applicant` validation reject the profile. The engine must refuse invalid input, which is a feature.
        - **Harness bug:** wrong mapping from `Applicant` to form values, or a jsdom event that didn't fire. To confirm, enter the same profile in the live calculator via Chrome; if the live page agrees with our engine, it's a harness bug. Fix the harness and add a harness self-test.
        - **Engine bug:** the live calculator and `criteries-scg.html` both disagree with our engine. Fix the engine and add the shrunk case as a golden profile.
        - **Official divergence:** the calculator disagrees with the criteria page, e.g. a rounding rule, an edge case or an outdated script. **No silent choice.**
          - Check the authoritative text (the Ministerial Instructions / regulations).
          - Escalate to the lawyer for a ruling, recorded in `fixtures/crs/divergences.md` with sources and date.
          - The fixture is marked `known_divergence` with the ruling, and the engine follows the ruling.
          - The fuzz harness skips only that exact pattern.
          - When a real client hits the case, the client note flags it.
          - Optionally report it to IRCC.
     3. **Re-run** the full fuzz with a new random seed after each fix, until a clean run.
8. **Test runner:**
   - `cargo test -p rules-engine` asserts each subtotal exactly and prints a factor diff on failure.
   - A fixture whose JS hash differs from the current one is flagged "stale: re-record".

**M0 exit:**
- All golden profiles match.
- Fuzzing has **0 unexplained mismatches** across 3 consecutive runs with different seeds (5,000 cases each). Every past mismatch is either fixed with a regression fixture, or a lawyer-ruled entry in `divergences.md`.
- Language boundary tests pass.
- Eligibility tests pass for each program.

---

## M1: Security skeleton
**Status 2026-10-18:** done; updater dormant and signing pending (last step); clean-machine VM test not run yet.

1. **Setup:**
   - Install rustup (stable + clippy + rustfmt), `cargo install tauri-cli --locked`, and fnm with Node LTS.
   - Note: the vendored OpenSSL build needs Perl on Windows CI. GitHub's runners have Strawberry Perl.
2. **Tauri shell, locked down:**
   - Tauri v2 with Svelte+Vite in `ui/`.
   - CSP `default-src 'self'`: no remote `connect-src`, no `unsafe-eval`. `withGlobalTauri: false`, no remote URLs.
   - Capabilities: our own commands plus `dialog:allow-open` and `dialog:allow-save` only. No fs, shell or http plugins exposed to the frontend.
3. **CI (`ci.yml`, macOS + Windows matrix):**
   - `fmt --check`, `clippy -D warnings`, `cargo test --workspace`;
   - the oracle fuzz job;
   - `cargo deny`/`cargo audit` and `npm audit --audit-level=high`;
   - the dependency-graph check;
   - an unsigned build.
4. **vault:**
   - Database: `rusqlite` with `bundled-sqlcipher-vendored-openssl`.
   - Keys: a random 256-bit DEK, wrapped by an Argon2id KEK derived from the passphrase. The wrapped DEK and salt are stored in the OS keychain (`keyring`).
   - Secrets are held in `secrecy`/`zeroize` types.
   - Migrations: `PRAGMA user_version` with ordered migrations, and an automatic encrypted backup (`sqlcipher_export`) before each one.
   - **Recovery key:** setup generates a second wrap of the DEK under a random recovery key (shown once as 24 words, to be printed and stored in the firm's safe). Encrypted backups carry the recovery-wrapped DEK, so a new laptop can restore with the backup plus the recovery key.
   - Tests:
     - a wrong passphrase fails;
     - the DB file contains no plaintext;
     - a migration takes a backup;
     - restoring with the recovery key works after the keychain entry is deleted.
5. **Auto-lock:**
   - A backend idle timer (default 5 min), reset by an `activity` command. The app also locks on OS sleep.
   - On lock: close the DB, zeroize the key, emit `locked`. Unlock and setup screens (setup shows the recovery key).
6. **net + audit:**
   - A single `reqwest` + `rustls` client: HTTPS only.
   - Allowlist of exact hosts: `api.anthropic.com`, `www.canada.ca`, `ircc.canada.ca`, plus the update host.
   - Redirects only to allowlisted hosts. Timeouts and a size cap.
   - Audit rows hold the time, host, data category, source file and status, never content. `tracing` never logs bodies, enforced by a CI lint.
   - Tests: a foreign host is rejected, `http` is rejected, a redirect to a foreign host is rejected, and an audit row is written.
7. **Updater:**
   - `tauri signer generate`; the private key stays offline / in GitHub secrets.
   - The user is prompted; restarts are never forced.
   - Stable and beta `latest.json` endpoints.
   - A custom `min_version` field makes the update mandatory.
   - `release.yml` builds with `tauri-action`. The update host is still a placeholder.
8. **Standalone guarantee:**
   - Windows `webviewInstallMode: offlineInstaller` (the `.msi` bundles WebView2) and `+crt-static` for `x86_64-pc-windows-msvc`.
   - macOS `universal-apple-darwin`.
   - SQLCipher and OpenSSL are vendored, and `rustls` handles HTTPS.
   - Future helper tools (OCR, etc.) are bundled as sidecars only.
   - Tauri's default WebView2 bootstrapper downloads at install time; `offlineInstaller` overrides that.
   - Unsigned builds run, but Gatekeeper and SmartScreen show warnings until notarization and the EV cert from step 9 are in place.
9. **Started early, outside the code:**
   - Apple Developer enrollment + notarization.
   - Windows EV code-signing cert.
   - `docs/threat-model.md`.

---

## M2: Client flow
**Status 2026-10-18:** de-identification and preview, mandatory review gate, PDF/Word export, input and output folders done; form parser waiting for a sample; Claude note deferred.

- Import from the input folder chosen in Paramètres (read on demand, one level, `.xlsx`/`.csv`, files never modified; tracked by SHA-256 so nothing is imported twice) into `Applicant`; reviewed reports go to the output folder. The format is still to be decided; the parser for it maps onto the canonical model.
- Optional Claude normalization of messy free-text answers (e.g. "BSc 2015, 4 yrs"). Claude **proposes** field values; the lawyer confirms them.
- De-identification with a preview of what will be sent.
- The engine computes the score and eligibility.
- Claude (no tools, zero data retention, forced schema) receives the de-identified Applicant, the engine breakdown and the grid citations, and returns the structured note draft (validated). Every note records the snapshot version, date and source URLs. The note includes the draw context: "client X vs latest cutoff Y in the same category".
- Review screen with source answers and cited rules → lawyer edits and approves → PDF/Word export.

## M2b: Daily watcher
**Status 2026-10-18:** built without the Claude parts.
- `rules-watcher` crate, vault v4 storage, daily run while unlocked plus "check now", the review screen with approve/reject, and draws in assessments;
- prose pages get a sentence diff only, until the Claude integration.

**Sources** (checked 2026-10-18):
| Source | Role | Extraction |
|---|---|---|
| `…/verifier-note/criteries-scg.html` (static tables, modified 2026-10-16) | **Reference CRS grid** | `grid-parser` (script) |
| `ircc.canada.ca/francais/immigrer/qualifie/scg-outil-udit.js` | Calculator logic, fetched as text and never executed in the app | Hash-watch. A change triggers re-running the dev oracle fuzz and re-recording the golden profiles, which tests the values without reading the code |
| `…/entree-express/modifications.html` | Announced changes with dates (e.g. job offer, 2026-11-03) | Diff of the text (script). **Claude** turns new prose into a proposed pending change with `effective_from` and quotes |
| `…/verifier-note/salaire-eleve.html`, `…/documents/offre-emploi.html` | Related rules | Script parse where tabular; otherwise diff + Claude summary |
| FSW/CEC/FST criteria pages, language equivalency charts | Eligibility + conversion | `grid-parser` (script) |
| `…/content/dam/ircc/documents/json/ee_rounds_123_fr.json` | Draws (number, date, type, size, minimum CRS) | serde only; stored automatically; notification sent |
| `verifier-note.html` form markup | Questions and options | Hash-watch → alert that the Applicant mapping may need updating |

**Flow:**
0. **Schedule:** the app runs the check after unlock if the last check was more than 24h ago, then every 24h while open, plus a "Check now" button in Settings.
1. Fetch through `net` (5 MB cap, HTML/JSON/JS/PDF content types only, audited) and skip unchanged hashes. **Hash the normalized `<main>` content, not the raw response:** canada.ca pages embed an Akamai beacon script with per-request tokens, so the raw bytes differ on every fetch even when nothing changed (observed 2026-10-18).
2. Store the raw response and its hash encrypted. These are evidence and future test fixtures.
3. Parse with scripts.
4. Validation:
   - totals and caps are consistent;
   - jump-size checks pass;
   - draw numbers increase and scores are in range.
5. **If a parser fails** (IRCC redesigned the page): keep the last approved snapshot and show a warning. Claude (no tools, forced schema, verbatim quotes that must appear in the page) proposes the values from the new layout. That proposal goes through the same diff and approval, and the developer then updates the parser.
6. **Comparison report**, one row per value: old / new / source / status, the cell text, plus an optional plain-language explanation from Claude.
7. **Approval gate:**
   - Run three checks against the candidate snapshot: the golden profiles, the oracle fuzz, and the live-calculator spot-check via Chrome (dev). Engine = official calculator = criteria page, or approval is blocked.
   - A future-dated snapshot (from `modifications.html`) can be approved ahead of its `effective_from`.
   - The lawyer approves, or overrides with a written reason that is recorded.
   - A stale warning shows when there has been no successful check for 7 days, or when the approved snapshot is older than the source page's last-modified date.
8. New links found in the reference page's `<main>` become "suggested sources" for the lawyer to add. The watcher never crawls.

**Separation:** the watcher never sees client data, and client analysis never fetches the web.

**Tests:**
- The saved sources serve as fixtures.
- Mutation tests:
  - changing a value must produce a diff;
  - renaming a table must make the parser fail loudly;
  - injected "ignore previous instructions" text in prose must leave Claude's proposal schema-valid and quote-checked, and it is never applied without approval.

---

## Development phase (decided 2026-10-18)
Work happens in **dev-only mode**: debug builds with the automatic test vault, plus the installed "LawyerTool Dev" app that is rebuilt after each change (README).

**Deferred until later (decided 2026-10-18): all Claude integration.**
- the drafted note, the normalization of imported answers, and the watcher's prose extraction;
- this also defers API key ownership, commercial terms and ZDR;
- everything else works without a key. The de-identified payload and its preview are ready, see M2.

**Very last step, before any real release:**
- switch on the updater: signing key, GitHub secrets, update host, `plugins.updater` config (docs/releasing.md);
- Apple notarization and Windows code signing.

The updater code is merged but dormant until then.

**GitHub Actions disabled (2026-10-18, to save plan minutes).** Meanwhile:
- `scripts/ci-local.sh` gates every commit (same checks as CI, **without Windows**);
- `scripts/post-merge-bump.sh` bumps the version, tags and pushes after each merge;
- re-enable with `gh workflow enable ci.yml` (and `release-version.yml`, `release.yml`) before any release, to get the Windows checks back.

## M3: Hardening
**Status 2026-10-18:** done, except the independent review.
- supply chain: `cargo audit` 0 vulnerabilities (Linux-only warnings accepted), one runtime npm dependency;
- storage: SQLite temp files in memory, input size limits;
- IPC: frozen permission/CSP test, `docs/ipc-surface.md`;
- hostile pages and feeds: parser and draws fuzzing, lookalike digits fail loudly, content-type allowlist;
- de-identification: invisible characters, homoglyphs, spaced names, spelled-out emails, property test;
- docs: threat model v2, `docs/security-review.md`.

## Open questions
- Client form export format: a sample with fake data is needed for the importer.
- Matching imported forms to existing clients (email / file number)?
- API key ownership (when the Claude integration resumes).
- Update host location (last step).

## Verification
- **M0:**
  - `cargo test -p rules-engine`: all golden profiles match on every subtotal.
  - `tools/oracle` fuzz: 0 unexplained mismatches over 3 seeds × 5,000 random profiles.
  - Spot-check 3 profiles live via Claude in Chrome.
  - Parser fixtures parse; a mutated fixture fails loudly.
  - Language boundary tests and eligibility tests pass.
- **M1:**
  - `cargo tauri dev`: passphrase setup shows the recovery key.
  - `strings` on the DB file shows no plaintext.
  - Idle auto-lock works.
  - A wrong passphrase is rejected.
  - Restore works with the recovery key after deleting the keychain entry.
  - In DevTools, `fetch('https://example.com')` is blocked by the CSP.
  - CI is green on macOS and Windows.
  - **Clean-machine test:** a fresh offline Windows 10 VM (no WebView2, no VC++ redistributable) and a fresh macOS VM (no developer tools) both install, launch, create the DB and lock. `otool -L` and `dumpbin /dependents` list only system libraries.
  - Updater: a higher-version `latest.json` triggers the prompt; `min_version` makes the update mandatory.
- **M2b:**
  - Mutated fixtures produce diffs or loud failures and are never applied silently.
  - A candidate snapshot that breaks a golden profile cannot be approved.
