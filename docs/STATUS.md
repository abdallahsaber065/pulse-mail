# Status: what works today

An honest snapshot of the code at `HEAD` (2026-10-21, v0.1.0, commits
`5e55575` → `13d7120`). It describes what the code does, not what is planned.
Plans live in [ROADMAP.md](ROADMAP.md) and [PLAN.md](PLAN.md).

Legend: **Done** = works end to end on the user's PC · **Partial** = works
but with a real gap · **Stubbed** = visible in the UI but does nothing yet ·
**Not started** = no code.

## Feature matrix

### Account and sign-in

| Feature | State | Notes | Code |
| --- | --- | --- | --- |
| Google sign-in (PKCE, loopback redirect) | Done | Opens the system browser; 5 min timeout | `src-tauri/src/auth/oauth.rs`, `auth/pkce.rs` |
| OAuth client file outside the repo | Done | `%APPDATA%\com.abdallah.pulsemail\google-oauth-client.json` or `PULSE_MAIL_OAUTH_CLIENT` | `auth/client_config.rs` |
| Refresh token in Windows Credential Manager | Done | Access token in memory only (`Secret`) | `auth/token_store.rs`, `auth/secret.rs`, `auth/mod.rs` |
| Resume on launch | Done | Silent when anything is missing | `commands.rs::resume` |
| Sign out | Done | In-app confirm dialog; revokes the token, deletes it and the account's cached mail | `commands.rs::sign_out`, `src/App.tsx` |
| Multiple accounts | Not started | Schema has `account_id` everywhere, but the engine, commands and UI all use `primary_account()` (lowest id). Signing in with a second account is not supported and leaves confusing state | `store/mod.rs::primary_account` |

### Sync

| Feature | State | Notes | Code |
| --- | --- | --- | --- |
| Fast first sync | Done | 50 newest INBOX ids + one Gmail batch of `format=metadata` gets; rows stream to the UI. ≈1.0 s in the benchmark, 2.4 s in the debug app | `sync/mod.rs::first_sync`, `sync/gmail.rs::get_metadata_batch` |
| Body prefetch | Done | Newest 12 INBOX messages get full bodies in the background | `sync/mod.rs::background` |
| Lazy bodies on open | Done | `load_thread` downloads missing bodies (`format=raw`); the reader shows the snippet + skeleton until then | `commands.rs::load_thread`, `Engine::load_thread_bodies` |
| Background backfill | Partial | Capped: `newer_than:90d -in:spam -in:trash`, at most 500 messages, metadata only. Older mail is never fetched. Done once per account (`backfill_done:<id>` setting) | `sync/mod.rs` (`BACKFILL_*`) |
| Incremental sync (history.list) | Done | Adds (fetched as `raw`), deletes, label changes; 404 → fresh first sync | `Engine::delta_sync`, `gmail.rs::HistoryDelta` |
| IMAP IDLE push | Done | INBOX only; re-IDLE every 20 min; backoff 1 → 60 s; 15 min pause after `invalid_grant` | `sync/imap_idle.rs` |
| Polling fallback | Done | 60 s while IMAP is down, 15 min safety poll while up | `sync/mod.rs` (`POLL_IMAP_*`) |
| Rate limits | Done | 429 and 403 `rateLimitExceeded` / `userRateLimitExceeded` retried with backoff (up to 3 retries); batches paced ≥ 1 s (250 units/s) | `gmail.rs::get`, `classify_error` |
| Panic isolation | Done | Every sync pass, the background job and the IMAP loop run under `catch_unwind`; status always resets | `Engine::run_once`, `spawn_background`, `imap_idle::run` |
| Labels and counts | Partial | Synced from Gmail on first sync and after new mail; local actions (read, archive) don't update the counts until the next sync | `Engine::sync_labels` |

### Reading and triage (UI)

| Feature | State | Notes | Code |
| --- | --- | --- | --- |
| Thread list | Partial | Virtualized, unread bold, OTP chip, density. **Shows at most 200 threads per label** (no paging on scroll yet) | `src/components/ThreadList.tsx`, `src/lib/api.ts::threads` |
| Reader | Done | Thread view, sandboxed HTML, image blocking, OTP banner with copy | `Reader.tsx`, `EmailFrame.tsx`, `lib/email-frame.ts` |
| Reading pane layout | Partial | Fixed right split at ≥ 1024 px; below that the reader replaces the list. No setting for other layouts | `src/App.tsx` |
| Archive, star, read/unread | Done | Hover row actions, reader action bar, shortcuts; optimistic with rollback; Undo for archive; mark read on open | `Engine::modify_thread`, `App.tsx::modify` |
| Archive outside the Inbox | Not started | Hidden on purpose outside INBOX | `App.tsx::actions` |
| Delete / trash, spam, snooze, labels editing | Not started | `modify_thread` already accepts TRASH/SPAM label ids | `commands.rs::modify_thread` |
| Search | Partial | Local SQLite FTS5 only (subject, sender, snippet, body). No Gmail operators, no server search, one row per message (not grouped by thread) | `store/mod.rs::search`, `fts_query` |
| Attachments | Not started | Only a `has_attachments` flag is stored; no list, download or `cid:` images | `sync/mime.rs` |
| Compose / reply / forward | Stubbed | Compose button is disabled with a tooltip | `Sidebar.tsx` |
| Keyboard shortcuts | Done | j/k, o/Enter, u/Esc, /, c, e, s, Shift+I, Shift+U, g+i/s/t/d, Shift+R, `,`, ? | `src/lib/shortcuts.ts` |

### Notifications and OTP

| Feature | State | Notes | Code |
| --- | --- | --- | --- |
| Windows toasts | Partial | New unread INBOX mail < 15 min old. Dev builds show **PowerShell** as the sender (no AUMID registered); installed build untested | `src-tauri/src/notify/mod.rs` |
| OTP extraction | Done | English + Arabic, Arabic-Indic digits, spaced/hyphenated codes, rejects promos/amounts/years/URLs; char-boundary safe (NBSP, emoji, Arabic tests) | `src-tauri/src/otp/mod.rs` |
| "Copy code" on toast, OTP chip, OTP banner | Done | One click; chip shows "Copied" | `notify/mod.rs`, `ThreadList.tsx`, `Reader.tsx` |
| Notification setting (all / codes only / off) | Done | Applies to incremental sync | `commands.rs::set_notifications`, `Engine::delta_sync` |
| Toast click opens the thread | Done | While the toast is on screen; not from Action Center after a restart (needs COM activation) | `window.rs::open_thread` |

### App shell and settings

| Feature | State | Notes | Code |
| --- | --- | --- | --- |
| Tray (Open, Check mail now, Quit) | Done | Left click opens the window | `src-tauri/src/tray.rs` |
| Start with Windows (`--minimized`) | Done | Setting in the UI | `tauri-plugin-autostart`, `commands.rs::set_autostart` |
| Close behavior (hide vs destroy) | Done | Setting in the UI; which default is better is not measured (ADR 0004) | `window.rs`, `SettingsPanel.tsx` |
| Single instance | Done | Second launch focuses the window | `lib.rs` |
| Theme (system/light/dark), density | Done | Stored in `localStorage` | `src/lib/theme.ts`, `src/lib/prefs.ts` |
| Sync transparency | Done | Top progress bar, status chip, activity popover with Retry | `SyncIndicator.tsx`, `lib/sync-view.ts`, `sync/status.rs` |
| In-app dialog and snackbars | Done | No `alert` / `confirm` / `prompt` anywhere | `components/ui/dialog.tsx`, `Snackbar.tsx` |
| Account menu | Done | From the avatar: settings, shortcuts, sign out | `AccountMenu.tsx` |
| App icon | Stubbed | Still the default Tauri icon | `src-tauri/icons/` |
| Installer (NSIS, per-user) | Partial | Configured, never built or installed on the user's PC | `tauri.conf.json` |
| Auto-update, code signing | Not started | | |

### Engineering

| Item | State | Notes |
| --- | --- | --- |
| Rust unit tests | Done | 56 tests, offline (OTP, OAuth, store, Gmail parsing, MIME, status) |
| UI unit tests | Done | 14 tests (vitest): formatting, email HTML sanitizing, status chip |
| Clippy / fmt | Done | Clean with `--all-targets` |
| Sync benchmark | Done | `cargo run --example sync_bench` (read-only Gmail calls) |
| CI | Not started | Local gates only |
| Component / end-to-end UI tests | Not started | UI checked by hand and with headless screenshots of the browser mock |

## Measured numbers

| What | Result | How |
| --- | --- | --- |
| First rows on a fresh sync, before (v0.1 path) | 8.8–20.0 s, 116 messages, ≈6 MB | `sync_bench` |
| First 50 rows, after | ≈1.0 s (list 0.4 s + one batch 0.6 s) | `sync_bench` |
| First 50 rows in the app | 2.4 s (debug build, includes token refresh + SQLite) | log `first page ready count=50 ms=2386` |
| Background backfill (100 messages) | 1.75 s | log `backfill done` |
| Bundle size | JS 126 KB (45 KB gzip), CSS 33 KB (7 KB gzip) | `pnpm build` |
| Toast latency, RAM, CPU, cold start | **Not measured** | see targets in [PLAN.md](PLAN.md) |

## Known bugs and limitations

1. **Search is local only.** Mail older than the 90-day / 500-message window is invisible, Gmail operators (`from:`, `has:attachment`) aren't understood, and results are per message, so one thread can appear more than once.
2. **List cap.** The UI asks for 200 threads per label and never pages further.
3. **Backfill cap.** 90 days, 500 messages, metadata only. No "load older mail" anywhere.
4. **Unread counts lag.** Sidebar counts come from Gmail's label totals; they update on the next sync, not on local actions.
5. **Dev toasts show PowerShell** as the sender and icon. Installed builds should use the app id, but that hasn't been checked.
6. **Toast latency and RAM are not measured.** There's no `toast shown` log line yet, so the latency method in PLAN.md can't be used as written.
7. **Compose is disabled.** No reply, forward or drafts.
8. **No attachments.** Can't see or save files; inline `cid:` images don't render.
9. **Single account only** (see the matrix).
10. **IMAP watches INBOX only.** Changes made elsewhere (for example in Gmail web) to other labels show up on the next poll, up to 15 minutes later.
11. **Bug: offline open never stops loading.** `load_thread` logs a failed body download and returns the cached rows, so the reader keeps showing the snippet and a loading skeleton with no error and no Retry. Reopening the thread tries again. Fix: return a "body unavailable" flag and show an inline retry.
12. **Sign out while the background backfill runs:** the job sees the stop flag at its next step and exits; a request already in flight still finishes.
13. **Cache not encrypted at rest; codes stay on the clipboard; codes can show on the lock screen** (see [SECURITY.md](SECURITY.md)).
14. **Default Tauri icon** in the tray, taskbar and installer.
15. **Hover row actions need a mouse** (Tailwind `hover` only applies on devices that support hover); keyboard shortcuts cover the same actions.
