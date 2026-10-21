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
- [x] Sync engine: full sync (60 days, 300 messages), history delta, 404 → full resync, 401 → token refresh and retry, empty-wake retry
- [x] IMAP IDLE doorbell over XOAUTH2 + rustls; re-IDLE every 20 min; reconnect with exponential backoff (1 s → 60 s); polling fallback 60 s when IMAP is down
- [x] Windows toasts via WinRT with "Copy code" and "Open" buttons; click opens that thread
- [x] Tray icon + menu (Open, Check mail now, Quit); single instance; start at login with `--minimized`
- [x] Window lifecycle: hide on close by default; destroy on close as a setting; app keeps running with no window
- [x] First-pass Gmail-like UI: top bar with search, sidebar with labels and unread counts, virtualized thread list with OTP chips, reader pane with sandboxed HTML, image blocking, OTP banner, light/dark, shortcuts, setup and sign-in screens, browser mock mode
- [ ] **User:** create Google OAuth client and save the JSON (see `SETUP-GOOGLE-OAUTH.md`)
- [ ] First real sign-in and full sync on the user's PC
- [ ] Measure toast latency (p50/p95) and fix whatever is slow
- [ ] Verify toasts and "Copy code" with an installed (NSIS) build: dev builds show the PowerShell name/icon
- [ ] Measure idle RAM in both close modes; pick the default (ADR 0004)
- [ ] Settings UI for: start with Windows, close behavior

Acceptance: a code sent to Gmail shows on a toast within the latency target,
"Copy code" puts it on the clipboard without opening a window, clicking the
toast opens that message, and it all still works after a sleep/resume and a
Wi-Fi drop.

### Phase 2: daily driver (read and triage)

- Mark read on open (Gmail `messages.modify`), star, archive, delete, mark unread, with optimistic UI
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

- **2026-10-21:** Phase 0 done. Phase 1 core implemented and compiling with
  unit tests; waiting on the user's OAuth client to run end to end.
