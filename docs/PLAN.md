# Pulse Mail: plan and progress

Phases with acceptance criteria, performance targets and the progress log.
What each item means in detail: [ROADMAP.md](ROADMAP.md). What works
today: [STATUS.md](STATUS.md). How we work: [WORKFLOW.md](WORKFLOW.md).

## Goal

Never open Gmail on the web again. A Windows tray app that is always on,
tells you about new mail within seconds (from every account), makes
verification codes one click away, and opens any email instantly. Looks like
Gmail, only cleaner. Fast and light: about 200 MB at most with the window open.

## Performance targets

Measured on the user's PC (release build unless noted, one Gmail account).
"p50/p95" are over at least 20 samples. Gaps: there's no `toast shown` log
line yet, so toast latency needs [ROADMAP M1](ROADMAP.md#m1) first.

| Metric | Target | How to measure | Measured |
| --- | --- | --- | --- |
| First 50 inbox rows on a fresh sign-in | ≤ 1.5 s | `cargo run --example sync_bench` (in `src-tauri`), or the activity line "Inbox ready: N newest messages in X s" | ≈1.0 s bench; 2.4 s in debug app ✅ |
| New-mail toast latency (Gmail receives it, toast shows) | p50 ≤ 3 s, p95 ≤ 6 s | Send a test mail with a timestamp in the subject; compare with the log line `toast shown` | not yet |
| Latency with IMAP down (fallback polling) | ≤ 65 s | Block imap.gmail.com in the firewall, repeat | not yet |
| Idle RAM, tray only (window destroyed) | ≤ 25 MB private working set for `pulse-mail.exe` | Task Manager → Details, or `Get-Process pulse-mail` (WS / PM) after 10 min idle | not yet |
| Idle RAM, window hidden (hide-on-close) | ≤ 120 MB total incl. WebView2 processes | Sum of `pulse-mail` + its `msedgewebview2` children | not yet |
| RAM, window open, reading mail | ≤ 200 MB total | Same as above while reading a large HTML mail | not yet |
| Idle CPU | ≈ 0 % (no wakeups beyond IDLE refresh every 20 min and the 15 min safety poll) | Process Explorer → CPU / context switches | not yet |
| Reopen window from tray, hidden mode | ≤ 100 ms to first paint | `performance.now()` stamp vs tray click log | not yet |
| Reopen window from tray, destroy mode | ≤ 600 ms to first paint | same | not yet |
| Toast "Copy code" → clipboard | ≤ 200 ms | manual | not yet |
| Thread list scroll | 60 fps with 10k rows | DevTools performance panel | not yet |
| Search in local cache | ≤ 50 ms for 10k messages | timing around the `search` command | not yet |
| Cold start at login to tray | ≤ 1.5 s, no window | log timestamps | not yet |
| Installer size | ≤ 10 MB | NSIS output | not yet |

## Phases

Each phase ends with every box checked, the gates in WORKFLOW.md green,
STATUS.md and this file updated, and the acceptance test done on the
user's PC. IDs (H1, A1, …) refer to [ROADMAP.md](ROADMAP.md).

### Phase 0: foundations ✅ (2026-10-21)

- [x] Project scaffold: Tauri v2 + SolidJS + TS + Tailwind v4, pnpm
- [x] Docs: README, AGENTS.md (+ CLAUDE.md pointer), PLAN, ARCHITECTURE, DESIGN, SECURITY, SETUP-GOOGLE-OAUTH, ADRs
- [x] `.gitignore` keeps secrets, databases and builds out of git

### Phase 1: notifications that never miss (nearly done)

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
- [ ] Add `toast shown` / first-paint / cold-start log lines, then measure toast latency (p50/p95) and fix whatever is slow ([M1](ROADMAP.md#m1))
- [ ] Verify toasts and "Copy code" with an installed (NSIS) build: dev builds show the PowerShell name/icon
- [ ] Measure idle RAM in both close modes; pick the default (ADR 0004)
- [x] Settings UI for: start with Windows, close behavior (plus theme, density, notifications all / codes only / off)
- [x] Transparency: top progress bar, status chip (Syncing n/m, Live, Up to date, offline, error + Retry), activity popover
- [x] No native `alert`/`confirm`/`prompt`: M3 dialog with scrim + animation, snackbar queue with Undo

Acceptance: a code sent to Gmail shows on a toast within the latency target,
"Copy code" puts it on the clipboard without opening a window, clicking the
toast opens that message, and it all still works after a sleep/resume and a
Wi-Fi drop.

### Phase 2: daily driver (read, find, triage, reply)

- [ ] List paging + reader body-error state (R3)
- [ ] Reading pane modes: right / bottom / no split / new window (R1)
- [ ] Search with Gmail operators, server search merged with local (S1), "load older" (H2)
- [ ] Trash, spam, archive everywhere, mute (T1); label picker (T2)
- [ ] Attachments: list, preview, save, inline `cid:` images (C4)
- [ ] Reply, reply all, forward, compose with drafts sync (C1); undo send (C2)
- [ ] Remote images per sender (V1)
- [x] Mark read on open, star, archive, mark unread with optimistic UI and Undo (done 2026-10-21)

Acceptance: a full working day of mail triage and replies without opening Gmail web.

### Phase 3: every account, all history

- [ ] Account registry; N accounts syncing and notifying at once (A1, ADR 0012)
- [ ] Account rail, Ctrl+1…9, unified inbox (A1, A2), per-account colors (A3)
- [ ] Per-account notification rules (N1)
- [ ] Resumable full-history backfill with progress and storage budget (H1, ADR 0014); DB maintenance (H3)

Acceptance: two accounts signed in; mail to the non-selected one toasts
within the latency target; switching accounts shows mail instantly; a full
mailbox backfill survives a restart and a sleep/resume.

### Phase 4: Windows citizen and polish

- [ ] App icon, AUMID registration, installed NSIS build tested, Action Center activation (W1)
- [ ] Tray unread badge (N3), quiet hours / Focus (N2), global hotkey (W3)
- [ ] OTP extras: auto-copy, clipboard clear, hide on lock screen, sender trust (O1)
- [ ] Command palette (K1); crash logs (V4); sleep/resume + network-change reconnect (X2)
- [ ] Settings page grown per U3

Acceptance: installed build used for a week with no dev console; every
target in the table above measured and met.

### Phase 5: lighter and faster

- [ ] Decide ADR 0004 from measurements; WebView2 `MemoryUsageTargetLevel = Low` when hidden
- [ ] Startup and idle profiling, release size check
- [ ] Optional: Gmail `users.watch` + Pub/Sub pull as a second push source (ADR 0002)
- [ ] Offline outbox (X1)

Acceptance: every performance target met and written down here.

### Later

Snooze (T3), send later (C2), categories (T4), unsubscribe helper (T7),
contacts autocomplete (C3), encryption at rest (V2), app lock (V3), auto-update
and signing (W2), `mailto:` handler (W4), i18n/RTL UI (U2), themes (U1), print
and export (R4), drag and drop (R5), mini window (R6), CI (M3).

## Progress log

- **2026-10-21 (night, 2):** `503baa5` history download v2 (stall fix, exact count,
  resumable queue, Retry), attachments policy and on-disk store, per-account page.
  `f0af3d7` one "Download to this PC" range (drops `offline_days`, migration 7).
  `498cf18` TipTap rich compose with attachments, forward files and signatures.
  Gates: 112 Rust tests, clippy clean, 38 vitest, build OK.
- **2026-10-21 (late night):** `5a375d5` migration 5. `79cfd04` taskbar
  overlay + tray unread badge; codes copied with clipboard-history/cloud
  exclusion formats. `a26a03b` reading pane modes (right/bottom/full page).
  `a5ee937` Gmail server search merged with local FTS (local operator
  parser). `31f6643` Ctrl+K command palette. `feee336` compose, reply,
  reply all, forward, Gmail drafts autosave, undo send, `mailto:`. Gates:
  107 Rust tests, 34 UI tests, clippy clean. DB encryption documented as
  the next step (SECURITY.md), not built.

- **2026-10-21 (night):** `66dcd8a` settings crash, infinite scroll, body
  retry. `15529c1` safe rendering (Rust sanitizer, sandboxed srcdoc frame,
  hash-pinned script, image proxy, trackers), attachments, reading prefs,
  IPC validation. `1e2bfdc` quiet background checks (ADR 0016). `d75fa4c`
  per-command IPC allowlist (ADR 0017). `38b1870` multi-account, unified
  inbox, onboarding, resumable history download (ADRs 0012, 0014, 0018).
  Migration 5 re-queues the real download for accounts the old capped
  backfill had marked done. Audits: `cargo audit` clean (5 Linux-only
  warnings), `pnpm audit` clean.

- **2026-10-21 (evening):** Docs refresh: STATUS.md (honest feature
  matrix), ROADMAP.md (prioritized catalog), WORKFLOW.md, ARCHITECTURE.md
  rewritten for the current code (sync state machine, IPC list, multi-account
  target), DESIGN.md (current UI + planned layouts), ADRs 0006–0014.

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
