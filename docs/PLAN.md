# Pulse Mail: plan and progress

## Goal

Never open Gmail on the web again. A Windows tray app that is always on,
tells you about new mail within seconds, makes verification codes one click
away, and opens any email instantly. Looks like Gmail, only cleaner.

## Performance targets

Measured on the user's PC (release build, one Gmail account, about 300
cached messages). "p50/p95" are over at least 20 samples.

| Metric | Target | How to measure |
| --- | --- | --- |
| First 50 inbox rows on a fresh sign-in | ≤ 1.5 s | `cargo run --example sync_bench` (in `src-tauri`), or the activity line "Inbox ready: N newest messages in X s" |
| New-mail toast latency (Gmail receives it, toast shows) | p50 ≤ 3 s, p95 ≤ 6 s | Send a test mail with a timestamp in the subject; compare with the log line `toast shown` |
| Latency with IMAP down (fallback polling) | ≤ 65 s | Block imap.gmail.com in the firewall, repeat |
| Idle RAM, tray only (window destroyed) | ≤ 25 MB private working set for `pulse-mail.exe` | Task Manager → Details, or `Get-Process pulse-mail` (WS / PM) after 10 min idle |
| Idle RAM, window hidden (hide-on-close) | ≤ 120 MB total incl. WebView2 processes | Sum of `pulse-mail` + its `msedgewebview2` children |
| RAM, window open, reading mail | ≤ 200 MB total | Same as above while reading a large HTML mail |
| Idle CPU | ≈ 0 % (no wakeups beyond IDLE refresh every 20 min and the 15 min safety poll) | Process Explorer → CPU / context switches |
| Reopen window from tray, hidden mode | ≤ 100 ms to first paint | `performance.now()` stamp vs tray click log |
| Reopen window from tray, destroy mode | ≤ 600 ms to first paint | same |
| Toast "Copy code" → clipboard | ≤ 200 ms | manual |
| Thread list scroll | 60 fps with 10k rows | DevTools performance panel |
| Search in local cache | ≤ 50 ms for 10k messages | timing around the `search` command |
| Cold start at login to tray | ≤ 1.5 s, no window | log timestamps |
| Installer size | ≤ 10 MB | NSIS output |

## Phases

Each phase ends with every box checked, `cargo test`, `pnpm test` and
`pnpm build` green, and this file updated.

### Phase 0: foundations ✅ (2026-10-21)

- [x] Project scaffold: Tauri v2 + SolidJS + TS + Tailwind v4, pnpm
- [x] Docs: README, AGENTS.md (+ CLAUDE.md pointer), PLAN, ARCHITECTURE, DESIGN, SECURITY, SETUP-GOOGLE-OAUTH, ADRs
- [x] `.gitignore` keeps secrets, databases and builds out of git

### Phase 1: notifications that never miss (in progress)

Goal: useful on day one even before the inbox UI is polished.

- [x] OAuth 2.0 installed-app flow: PKCE S256, loopback `127.0.0.1:<random>`, state check, 5 min timeout
- [x] Client config read from a local file outside the repo (`%APPDATA%\com.abdallah.pulsemail\google-oauth-client.json`)
- [x] Refresh token in Windows Credential Manager (`keyring`), access token in memory only, `Secret` type that can't be logged
- [x] SQLite cache with migrations, FTS5 search, label table, settings table
- [x] OTP extractor (English + Arabic, Arabic-Indic digits, spaced and hyphenated codes, rejects promos, amounts, years, URLs) with unit tests
- [x] Gmail REST client: profile, labels, messages.list, messages.get (raw), history.list with paging; history folding into a delta (unit tested)
- [x] Sync engine: history delta, 404 → full resync, 401 → token refresh and retry, empty-wake retry
- [x] Fast first sync (2026-10-21): 50 newest INBOX messages as metadata in one Gmail batch request, rows stream to the UI; bodies prefetched for the top 12 and loaded lazily on open; older mail (90 days, up to 500) backfilled in the background at ≤ 250 quota units/s
- [x] Sync resilience: each pass runs under `catch_unwind`, status always resets, panics are logged and retried; 429 / 403 rate-limit → backoff and retry
- [x] IMAP IDLE doorbell over XOAUTH2 + rustls; re-IDLE every 20 min; reconnect with exponential backoff (1 s → 60 s); polling fallback 60 s when IMAP is down
- [x] Windows toasts via WinRT with "Copy code" and "Open" buttons; click opens that thread
- [x] Tray icon + menu (Open, Check mail now, Quit); single instance; start at login with `--minimized`
- [x] Window lifecycle: hide on close by default; destroy on close as a setting; app keeps running with no window
- [x] First-pass Gmail-like UI: top bar with search, sidebar with labels and unread counts, virtualized thread list with OTP chips, reader pane with sandboxed HTML, image blocking, OTP banner, light/dark, shortcuts, setup and sign-in screens, browser mock mode
- [x] **User:** create Google OAuth client and save the JSON (see `SETUP-GOOGLE-OAUTH.md`)
- [x] First real sign-in and full sync on the user's PC
- [ ] Measure toast latency (p50/p95) and fix whatever is slow
- [ ] Verify toasts and "Copy code" with an installed (NSIS) build: dev builds show the PowerShell name/icon
- [ ] Measure idle RAM in both close modes; pick the default (ADR 0004)
- [x] Settings UI for: start with Windows, close behavior (plus theme, density, notifications all / codes only / off)
- [x] Transparency: top progress bar, status chip (Syncing n/m, Live, Up to date, offline, error + Retry), activity popover
- [x] No native `alert`/`confirm`/`prompt`: M3 dialog with scrim + animation, snackbar queue with Undo

Acceptance: a code sent to Gmail shows on a toast within the latency target,
"Copy code" puts it on the clipboard without opening a window, clicking the
toast opens that message, and it all still works after a sleep/resume and a
Wi-Fi drop.

### Phase 2: daily driver (read and triage)

- [x] Mark read on open, star, archive, mark unread (Gmail `threads.modify`), optimistic with rollback, Undo for archive; hover row actions + reader action bar + shortcuts (e, s, Shift+I, Shift+U)
- Delete (trash), snooze
- Thread view polish: collapse older messages, quoted text folding, attachments list with save
- Inline `cid:` images from attachments; "always show images from this sender"
- Search: local FTS first, then "search all mail on Gmail" fallback
- Categories (Primary / Updates / Promotions) tabs like Gmail
- Notification rules: quiet hours, VIP senders, mute labels, OTP-only mode
- Lock-screen privacy option: hide codes on the lock screen
- Paging deeper into history on scroll (older than the 60-day window)
- Acceptance: a full day of mail triage without opening Gmail web

### Phase 3: lighter and faster

- Measure and trim: WebView2 `MemoryUsageTargetLevel = Low` when hidden, lazy-load the reader, cap the HTML cache
- Optional: Gmail `users.watch` + Pub/Sub pull subscription as a second push source (ADR 0002)
- Startup and idle profiling, release build size check
- Acceptance: every target in the table above is met and written down here

### Phase 4: compose

- Compose, reply, reply all, forward (plain + basic rich text), drafts synced to Gmail
- Send via Gmail API `messages.send`; undo send (delayed send window)
- Acceptance: reply to a thread and it shows correctly in Gmail web

### Later / maybe

- Multiple accounts (schema already has `account_id`)
- Snooze, scheduled send, labels editing
- Auto-update via Tauri updater (signed)

## Progress log

- **2026-10-21 (later):** Fixed the OTP panic (byte index inside a NBSP in
  `in_url_token`; it killed the first sync, leaving 7 messages and a stuck
  "Checking for mail…"). Rewrote first sync. Measured with
  `examples/sync_bench` on the user's PC/account: old path (list 60 days, `raw`
  at 6 in flight, UI refresh only at the end) showed rows after 8.8–20.0 s
  (116 messages, 5.7–6.1 MB); new path lists the 50 newest INBOX ids (0.4 s)
  and fetches their metadata in one batch call (0.6 s), so the first 50 rows
  show after ≈1.0 s. Single metadata gets would take 2.6–3.2 s. In the running app (debug build, including token refresh, profile and SQLite writes) the log said `first page ready count=50 ms=2386`, then `backfill done count=100 ms=1750`. UI: dialog,
  snackbars with Undo, account menu, settings sheet, sync chip + activity,
  skeletons, empty states, density, hover actions.
- **2026-10-21:** Phase 0 done. Phase 1 core implemented and compiling with
  unit tests; waiting on the user's OAuth client to run end to end.
