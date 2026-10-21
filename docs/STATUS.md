# Status: what works today

An honest snapshot of the code at `HEAD` (2026-10-21, v0.1.0, through commit
`38b1870` plus later commits listed in [PLAN.md](PLAN.md#progress-log)). It
describes what the code does, not what is planned. Plans live in
[ROADMAP.md](ROADMAP.md) and [PLAN.md](PLAN.md).

Legend: **Done** = works end to end on the user's PC · **Partial** = works
but with a real gap · **Stubbed** = visible in the UI but does nothing yet ·
**Not started** = no code · **Untested live** = unit-tested and built, not
yet exercised against the real account.

## Defaults chosen for the user (2026-10-21)

| Topic | Default | Where it can change |
| --- | --- | --- |
| Reading pane | Right of the list; full-page option | Settings → Reading (planned R1) |
| Offline bodies | Last 90 days; older on demand; 1 GB budget | Settings → Storage (`offline_days`, `storage_budget_mb`) |
| Accounts | Several accounts, per-account inboxes + "All inboxes"; every account notifies | Settings → Accounts (per-account notifications) |
| Shortcuts | `c` compose, `Shift+C` copy code | `src/lib/shortcuts.ts` |
| Codes | Not auto-copied (`otp_autocopy=0`); clipboard cleared after 60 s; codes hidden on the lock screen | Settings → Codes |
| Close button | Hides to the tray | Settings → General |
| Google scopes | No new scopes (`https://mail.google.com/` only) | — |
| Language | English UI; mail text uses `dir=auto` for RTL | — |
| Snooze / send later | Local-only, behind a setting, later | ROADMAP T3 / C2 |

## Feature matrix

### Account and sign-in

| Feature | State | Notes | Code |
| --- | --- | --- | --- |
| Google sign-in (PKCE, loopback redirect) | Done | Opens the system browser; 5 min timeout | `auth/oauth.rs`, `auth/pkce.rs` |
| OAuth client file outside the repo | Done | `%APPDATA%\com.abdallah.pulsemail\google-oauth-client.json` or `PULSE_MAIL_OAUTH_CLIENT` | `auth/client_config.rs` |
| Refresh token in Windows Credential Manager | Done | One entry per account (`refresh-token:<email>`); access token in memory only (`Secret`) | `auth/token_store.rs`, `auth/secret.rs` |
| Resume on launch | Done | Starts every signed-in account | `commands/mod.rs::resume` |
| Multiple accounts | Done (untested live with a 2nd account) | One sync engine, IMAP IDLE task and status cell per account. Account menu lists accounts with unread counts, "All inboxes", "Add another account". Commands take `accountId` (null = default account, or all accounts for list/search) | `lib.rs` (`AppState.syncs`, `statuses`), `AccountMenu.tsx` |
| Unified inbox | Done | "All inboxes" merges every account by date; actions route to the row's account | `store::list_threads(None)`, `App.tsx` |
| Onboarding (first run and "Add account") | Done (untested live) | Welcome → Google sign-in → download range (30 days / 90 days / 1 year / everything) with size estimates → notifications (all / codes only / off) → live progress (n/total, phase, speed, ETA, pause/resume, "Continue to inbox") → done. A half-finished setup resumes after a restart (`backfill_state='setup'`) | `components/Onboarding.tsx`, `commands::account_estimates`, `finish_onboarding` |
| Remove account | Done | Settings → Accounts → Remove (confirm dialog): revokes, deletes the token and that account's cached mail. Never done automatically | `commands::sign_out(accountId)` |

### Sync

| Feature | State | Notes | Code |
| --- | --- | --- | --- |
| Fast first sync | Done | 50 newest INBOX ids + one Gmail batch of `format=metadata`; rows stream to the UI. ≈1.0 s in the benchmark, 2.4 s in the debug app | `sync/mod.rs::first_sync` |
| History download (H1) | Done (untested live beyond the existing account) | Per account and range: `messages.list` pages of 500 (cursor saved after each page, so it resumes after restart), metadata through the batch path; then offline bodies newer than `offline_days`, stopping at the storage budget. Pause/resume. Progress: phase, n/total, speed, ETA in the onboarding screen, Settings → Accounts and the chip popover | `Engine::history_download`, `HistSession`, migration 4 |
| Lazy bodies on open | Done | Older bodies download when opened | `commands::load_thread` |
| Incremental sync (history.list) | Done | Adds, deletes, label changes; history 404 → fresh first sync and re-download of the range | `Engine::delta_sync` |
| IMAP IDLE push | Done | INBOX only, per account; re-IDLE every 20 min; backoff 1 → 60 s | `sync/imap_idle.rs` |
| Polling fallback | Done | 60 s while IMAP is down, 15 min safety poll while up | `sync/mod.rs` |
| Quiet background checks | Done | Push deltas, polls and retries don't show the top bar or change the chip ("Live · now" / "Live · 2m"). They appear only in the chip popover's activity list. One failed background check is logged, not shown; the chip shows an error after two in a row or for sign-in problems. Catch-ups of ≥ 25 messages count as long work | `SyncStatus.foreground`, `LOUD_DELTA`, `lib/sync-view.ts` |
| Top progress bar | Done | Only for user-started or long work (first sync, history download, manual refresh). Appears after 400 ms, stays at least 600 ms | `SyncIndicator.tsx::TopProgress` |
| Rate limits | Done | 429/403 rate reasons retried with backoff; batches paced ≥ 1 s | `gmail.rs` |
| Panic isolation | Done | Sync passes, background job and IMAP loop run under `catch_unwind` | ADR 0007 |
| Labels and counts | Partial | Synced from Gmail; local actions update counts only on the next sync | `Engine::sync_labels` |

### Reading and rendering

| Feature | State | Notes | Code |
| --- | --- | --- | --- |
| Thread list | Done | Virtualized, pages by 200 on scroll, unread bold, OTP chip, density | `ThreadList.tsx` |
| HTML rendering | Done | Rust sanitizer (ammonia allowlist + CSS sanitizer) → `srcdoc` iframe `sandbox="allow-scripts"` (opaque origin) with a frame CSP whose only script is our SHA-256-pinned helper (auto-height, link interception, quote folding "•••"). Verified in Chrome that injected scripts/handlers don't run | `render/`, `EmailFrame.tsx`, ADR 0015 |
| Remote images | Done | Blocked by default with a bar; "Show images" / always for this sender. Fetched by Rust through a privacy proxy (public IPs only, no cookies or referrer, type sniffing, size caps), inlined as `data:`; known trackers dropped. Blocked images keep a light placeholder instead of a large gap | `render/proxy.rs` |
| Dark mode for mail | Done | Smart dark, or the sender's original colors | `email_dark` pref |
| Links | Done | Opened in the system browser via `open_external` (http/https/mailto only); warns when the visible text names a different domain | `lib/links.ts` |
| Attachments | Done | Listed with size/type; Open / Save (Mark-of-the-Web written; dangerous types are never opened directly); `cid:` inline images render | `commands/mail.rs`, `sync/mime.rs` |
| Print | Done | From the reader | `Reader.tsx` |
| Reading pane modes | Partial | Right split at ≥ 1024 px, reader replaces the list below that. Right / full-page setting is planned (R1) | `App.tsx` |
| Archive, star, read/unread | Done | Optimistic with rollback; Undo for archive | `Engine::modify_thread` |
| Delete / trash, spam, snooze, label editing | Not started | | |
| Search | Partial | Local FTS5 across all cached accounts, grouped by thread. No server search or Gmail operators yet (S1) | `store::search` |
| Compose / reply / forward | Stubbed | Compose button disabled | `Sidebar.tsx` |
| Keyboard shortcuts | Done | j/k, o/Enter, u/Esc, /, e, s, Shift+I/U, g+i/s/t/d, Shift+R, `,`, ?, Shift+C copy code | `lib/shortcuts.ts` |

### Notifications and codes

| Feature | State | Notes | Code |
| --- | --- | --- | --- |
| Windows toasts | Partial | New unread INBOX mail < 15 min old, per account; with several accounts the toast says "To <account>". Dev builds show PowerShell as the sender (no AUMID) | `notify/mod.rs` |
| Notification mode per account | Done | All / codes only / off, chosen in onboarding and in Settings → Accounts | `accounts.notify` |
| OTP extraction | Done | English + Arabic, Arabic-Indic digits, char-boundary safe | `otp/mod.rs` |
| Copy code (toast, chip, banner, Shift+C) | Done | Clipboard cleared after 60 s if unchanged (setting); auto-copy off by default (setting) | `lib.rs::copy_code` |
| Codes off the lock screen | Done | While the session is locked the toast hides the code | `notify::session_locked` |

### App shell and settings

| Feature | State | Notes |
| --- | --- | --- |
| Tray (Open, Check mail now, Quit) | Done | "Check mail now" checks every account |
| Start with Windows, close = hide, single instance | Done | |
| Settings sheet | Done | General, Appearance, Reading (images, dark mail), Codes, Accounts (range, notifications, history progress, pause, remove), Storage (offline days, budget, size used), About |
| Theme, density | Done | `localStorage` |
| Sync chip + popover | Done | Per-account history rows with pause/resume; activity list names the account |
| App icon | Stubbed | Default Tauri icon |
| Installer (NSIS, per-user) | Partial | Configured, never installed on the PC |
| Tray unread badge, Ctrl+K palette | Not started | |
| Auto-update, code signing | Not started | |

### Engineering

| Item | State | Notes |
| --- | --- | --- |
| Rust unit tests | Done | 94 tests, offline |
| UI unit tests | Done | 25 tests (vitest) |
| Clippy / fmt | Done | `-D warnings`, `--all-targets` |
| Security invariant tests | Done | IPC allowlist in sync, devtools off in release, log calls never format secrets (`security.rs`) |
| Dependency audit | Done | `cargo audit`: no vulnerabilities; 5 allowed warnings, all Linux-only GTK/zbus crates not built on Windows. `pnpm audit`: none |
| CI | Not started | Local gates only |

## Measured numbers

| What | Result | How |
| --- | --- | --- |
| First rows on a fresh sync, old path | 8.8–20.0 s, 116 messages, ≈6 MB | `sync_bench` |
| First 50 rows, new path | ≈1.0 s | `sync_bench` |
| First 50 rows in the app | 2.4 s (debug build) | log |
| Toast latency, RAM, CPU, cold start | **Not measured** | see [PLAN.md](PLAN.md) |

## Known bugs and limitations

1. **Search is local only** (S1): no Gmail operators, no server results for mail outside the downloaded range.
2. **Second account untested live.** The multi-account path is unit-tested and the existing account migrated (range 90 days); adding a real second account hasn't been tried.
3. **Unread counts lag** local actions until the next sync.
4. **Dev toasts show PowerShell** as the sender.
5. **Compose disabled**; no reply, forward or drafts yet.
6. **IMAP watches INBOX only**; other labels catch up on the 15-min poll.
7. **Cache not encrypted at rest** (relies on the Windows account / BitLocker). Next step: SQLCipher with a DPAPI-protected key (V2).
8. **Default Tauri icon**; no tray unread badge yet.
9. **Body eviction** above the storage budget isn't implemented; the download simply stops adding bodies at the budget.
