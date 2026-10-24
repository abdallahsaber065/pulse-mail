# ADR 0027: Self-update via GitHub Releases and tauri-plugin-updater

- Date: 2026-10-24
- Status: accepted
- Scope: ROADMAP W2 (auto-update), requested by the maintainer

## Context

The app ships installers from GitHub Releases, but updating meant manually
downloading and running the new installer. The maintainer asked for: an
update check, automatic download, a notification, and settings to control
both the check and the download.

Two hard rules shaped the design: no new background polling loops (rule 6),
and network endpoints beyond Google's Gmail/OAuth require an explicit ask
(rule 8) — this feature is that ask, recorded here.

## Decision

**Mechanism**: `tauri-plugin-updater` (the standard Tauri v2 path) against
`https://github.com/abdallahsaber065/pulse-mail/releases/latest/download/latest.json`.
`tauri-action` builds the installers, minisign-signs them (private key +
password in GitHub Secrets `TAURI_SIGNING_PRIVATE_KEY[_PASSWORD]`, public
key pinned in `tauri.conf.json`), generates `latest.json`, and uploads both
to the release when `bundle.createUpdaterArtifacts` is on. Updates are
signature-verified before install; the Windows NSIS install runs in
"passive" mode.

**Behavior** (all in Rust, `update.rs`; the UI stays a stateless renderer):
- One quiet check 45 s after launch — never a polling loop — plus a manual
  "Check for updates" button in Settings → About.
- Prefs `update_check` (default on) and `update_auto_download` (default on,
  per the maintainer's request) gate the check and the background download.
- Notifications are snackbars, not native dialogs: "X is available" with an
  Install action, "Downloading…" while the artifact fetches, and "Restart
  to install" once the signature-verified bytes are in memory.
- Installing uses the downloaded bytes when present (instant), otherwise
  downloads now. Windows: the installer launches (passive) and exits the
  app. macOS/Linux: the plugin swaps the app and we restart.

**Platform notes (honest v1)**:
- Linux: the updater covers **AppImage** installs; `.deb`/`.rpm` users
  update by installing the new package (the check may fail there — it is
  logged and surfaced as a normal error).
- macOS: the app is unsigned, so the first launch after an update may
  re-trigger Gatekeeper (right-click → Open). Proper signing is W2 work.
- The updater downloads from GitHub over HTTPS only, and only ever contacts
  `github.com/abdallahsaber065/pulse-mail` — no telemetry, no version
  pings beyond the manifest fetch.

## Consequences

- Losing the minisign private key (or password) means future updates can't
  be signed; the key lives in GitHub Secrets and a local backup at
  `~/.tauri/` that the maintainer must keep.
- The first release that carries the updater (v0.4.0) cannot self-update —
  from v0.5.0 on, every tag flows to installed apps automatically.
- VERIFY-LIVE gains an updates section that can only be exercised once two
  releases exist.
