# Releasing and updates

## What is automatic

- **Every merge into `main`** bumps the version (patch by default; PR label
  `release:minor` or `release:major`), commits `chore(release): vX.Y.Z` and
  tags `vX.Y.Z` (`.github/workflows/release-version.yml`).
- **Minor and major releases** also run `.github/workflows/release.yml`:
  installers (universal macOS `.app`/`.dmg`, Windows `.msi`) on a **draft**
  GitHub release. Any other tag: Actions → Release → Run workflow → tag.
- With the signing secret set, the release also contains the signed update
  bundles and `latest.json`, with `min_version` taken from
  `release/min-version`.

## In the app

- The updater is **off until configured** (`plugins.updater` in
  `src-tauri/tauri.conf.json`). Until then, "check for updates" reports
  *not configured* and nothing is contacted.
- Once configured: after unlock, the app checks the endpoint of its channel
  (`stable`, or `beta` when the vault setting `update_channel` is `beta`).
  The endpoint host must be on the allowlist (`ALLOWED_HOSTS`), and every
  check and download is in the audit log.
- The user is asked; the app never restarts on its own. Below `min_version`,
  the prompt is mandatory (full-screen, no "later").
- The download is verified against the public key compiled into the app; the
  vault is locked (key wiped) before restarting into the new version.

## One-time setup (owner)

### 1. Signing key (you, never shared)

```sh
node ui/node_modules/@tauri-apps/cli/tauri.js signer generate -w ~/.tauri/lawyertool-updater.key
```

Choose a strong password. Keep `~/.tauri/lawyertool-updater.key` offline
(password manager / encrypted USB). The command prints the **public key**.

Store the private key and password as GitHub secrets (values are read from
your terminal, not typed into chat):

```sh
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/lawyertool-updater.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD
```

Losing the private key means installed apps can no longer be updated
(a new key needs a manual reinstall). Back it up.

### 2. Update host

The updater downloads `latest.json` and the bundles anonymously, so they
cannot stay on this private repository. Pick one:

- a separate **public** repository for releases (simplest), or
- a storage bucket (Cloudflare R2, S3) or the firm's web server.

Layout: `https://<host>/lawyertool/stable/latest.json` and
`https://<host>/lawyertool/beta/latest.json`, plus the bundles the JSON
points to. Publishing the draft release's files there is the last step to
automate once the host is chosen.

### 3. Configure the app

In `src-tauri/tauri.conf.json`:

```json
"plugins": {
  "updater": {
    "pubkey": "<public key from step 1>",
    "endpoints": ["https://<host>/lawyertool/stable/latest.json"],
    "requireSignedVersion": true
  }
}
```

Add `<host>` to `ALLOWED_HOSTS` in `src-tauri/src/session.rs`. Merge: the next
build ships the updater.

## Mandatory updates

Raise `release/min-version` (e.g. `0.4.2`) in the PR that fixes a critical
issue. Every older install then shows a mandatory update.

## Code signing (installers)

- **macOS:** Apple Developer Program, a "Developer ID Application"
  certificate, notarization. Secrets: `APPLE_CERTIFICATE` (base64 .p12),
  `APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
  `APPLE_PASSWORD` (app-specific), `APPLE_TEAM_ID`; then add them to the
  `tauri-action` step env in `release.yml`.
- **Windows:** an EV code-signing certificate (SmartScreen reputation);
  configure `bundle.windows.signCommand` for the certificate provider.

Until then, installers are unsigned: macOS needs right-click → Open, Windows
SmartScreen shows "More info → Run anyway".
