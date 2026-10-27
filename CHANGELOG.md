# Changelog

All notable changes to Pulse Mail are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/); versions are `0.x.y` until
the 1.0 hardening milestone. Released binaries are on
[GitHub Releases](https://github.com/abdallahsaber065/pulse-mail/releases).

## [Unreleased]

### Added

- Demo mailbox for contributors: `pnpm dev` (or `PULSE_MAIL_DEMO=1 pnpm tauri
  dev`) seeds fictional mail — including a Northwind OTP thread — without
  touching Google, the keychain, or the on-disk cache. `?toast=1` shows a
  stand-in verification-code notification. Off in every release build.
- Release builds can embed the project's Google OAuth Desktop client from
  repository secrets (`PULSE_GOOGLE_CLIENT_ID` / `PULSE_GOOGLE_CLIENT_SECRET`).
  A local `google-oauth-client.json` still wins. See [docs/RELEASING.md](docs/RELEASING.md).
- Public landing site (GitHub Pages): home, privacy policy, and terms at
  https://abdallahsaber065.github.io/pulse-mail/ — no cookies or analytics.
  Owner checklist: [docs/GOOGLE-VERIFICATION.md](docs/GOOGLE-VERIFICATION.md).

### Changed

- README rewritten for the public launch: demo GIF, download links above the
  fold, an honest OAuth / ~100-user note, and a short privacy section.
  Prebuilt installers embed the project's Desktop client (unverified);
  source builds and a local JSON still use your own.

## [0.4.0] — 2026-10-24

### Added

- **Self-update** (Settings → About): Pulse Mail checks GitHub Releases for
  a newer version once after launch (or on demand), can download it in the
  background, and notifies you — installing always waits for your click.
  Artifacts are signature-verified. Both behaviors are configurable, and
  the only server contacted is this project's own GitHub Releases (ADR 0027).

- **Linux and macOS (Apple Silicon) support**: the app now builds, tests and
  ships for all three platforms on every release. Linux gets freedesktop
  notifications with "Copy code" / "Open" actions (pure-Rust zbus, no
  libdbus); macOS gets banner notifications (buttons need a signed bundle —
  later), a dock unread badge (already present) and the full sync engine.
  Quiet hours use the system local time on every platform via
  `localtime_r` on unix. CI compiles and tests all three platforms on every
  PR; Apple Intel (x86_64 Mac) is excluded from the pipelines.

### Changed

- **Search no longer freezes the app**: list, search and tab-count queries
  run on a background thread instead of the UI's main thread, and SQLite's
  page cache grew to 20 MB so repeated searches stay fast on big mailboxes
  (the tray-only RAM target moved to ≤ 50 MB to match).
- **Search starts at two characters** (like Gmail; one CJK/Arabic character
  still works), so the first keystroke no longer triggers the heaviest scan
  for the least information. Results, ordering and paging are unchanged
  while large result sets now paginate through a much cheaper query plan.
- CI caches Rust builds even when a job fails (`cache-on-failure`), so
  fix-and-retry loops start warm.

## [0.3.0] — 2026-10-24

A reliability, rendering, and performance release: quieter nights, faster
view switches, honest dark mode, and an unsubscribe shortcut.

### Added

- **Quiet hours** (Settings → Notifications): pause notification toasts
  between two local times (30-minute steps, windows may cross midnight).
  Mail keeps syncing and counts stay fresh; verification codes still toast
  unless opted out, because they are time-critical. (ADR 0026)
- **Sleep/resume catch-up**: a clock-jump detector notices when the PC woke
  (wall clock ahead of monotonic time by ≥ 90 s) and immediately runs a
  quiet background sync on every account instead of waiting out the IMAP
  reconnect backoff. (ADR 0026; the IMAP re-handshake itself still follows
  its backoff)
- **Local crash logs**: rolling daily log files in
  `%LOCALAPPDATA%\com.abdallah.pulsemail\logs`, pruned to 7 days, with a
  panic hook recording every panic. Settings → About → "Open logs folder".
  Logs never contain message bodies or tokens. (ADR 0026)
- **Unsubscribe button** (reader header): messages carrying a
  `List-Unsubscribe` header offer one click to the sender's web link
  (system browser) or mailto (pre-filled compose), through the usual
  phishing-checked link path. The RFC 8058 one-click POST is deliberately
  not implemented (it contacts arbitrary sender endpoints).
- **Measurement tooling**: `startup ready` / `window shown` / `toast shown`
  log markers and `scripts/measure.ps1` (app + WebView2 memory sampling)
  for the performance targets in `docs/PLAN.md`.

### Changed

- **Faster view switches**: the first page of a folder/tab is now 50 rows
  (older rows still load on scroll), and the last 8 view results stay
  cached in memory — revisiting a category, label or account renders
  instantly while a background refresh updates unread flags.
- **Search typing is calmer**: the selection and list scroll no longer
  reset on every keystroke, only on an executed view change.
- **Quieter read-on-open**: opening a conversation (row click, toast,
  deep link) marks it read without a snackbar; explicit mark-read actions
  keep their confirmation.
- Database: `PRAGMA optimize` + FTS5 incremental merge run after sync
  passes that changed something, keeping search fast on long uptimes.
- CI is a single Windows job (matching the shipping platform) with
  `cargo fmt --check` and `pnpm audit` gates; the release pipeline builds
  the Windows NSIS installer. Native macOS/Linux support is not implemented
  yet (planned next).

### Fixed

- **No more wrong-content flashes when switching**: the reader can no
  longer show the previous thread's messages under a new selection, the
  list no longer shows the previous folder's rows mid-load, a reused
  message row drops the old body immediately, and the reader scroll resets
  per thread while same-thread refreshes keep the reading position.
- **Dark mode for mail is readable in both modes**: smart dark no longer
  renders washed-out light text on a white canvas (the invert filter does
  not apply to the propagated canvas background — the canvas is now set to
  its final dark color directly), and "original colors" renders white mail
  as a white card instead of dark-on-dark.
- **Opening mail from a notification now marks it read** (row clicks
  already did; the toast and deep-link paths now share the same behavior).
- Security: `xlsx` upgraded 0.18.5 → 0.20.3 from the SheetJS CDN, clearing
  two high advisories (ReDoS, prototype pollution) in the untrusted
  attachment preview path. `pnpm audit` is clean.
- Repository-wide rustfmt drift cleared and gated in CI.

## [0.2.1] — 2026-10-24

- Default "All" inbox tab, dark reader contrast tuning, non-sensitive demo
  data, GitHub Sponsors, project-level OAuth client support.

## [0.2.0] — 2026-10-23

- Brand icon suite, on-demand attachment previews (images/PDF/Office/media,
  zero temp files), undo send grace window, contact autocomplete in
  compose, thread context menu, inline quick reply, resizable split view,
  search syntax guide, Gmail draft editing.
