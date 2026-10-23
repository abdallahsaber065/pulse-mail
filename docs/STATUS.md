# Status: what works today

## Current Release: v0.2.1 (2026-10-24)

- **Always-on reliability (2026-10-24, ADR 0026):**
  **Quiet hours (N2)** — Settings → Notifications, a local-time window
  (30-minute steps, may cross midnight) that pauses non-code toasts while
  mail keeps syncing; codes stay allowed by default (setting). Local time
  via `GetDynamicTimeZoneInformation`, boundary logic unit-tested.
  **Sleep/resume catch-up (X2, partial)** — a clock-jump detector
  (wall vs monotonic ≥ 90 s) triggers a quiet `sync_now(false)` on every
  account right after the PC wakes, instead of waiting out the IMAP
  backoff; IMAP re-handshaking itself still rides the existing backoff.
  **Crash logs (V4)** — rolling daily log files in
  `%LOCALAPPDATA%\com.abdallah.pulsemail\logs` (7-day retention, no
  message contents or tokens), a panic hook that records every panic, and
  Settings → About → "Open logs folder" (`open_logs_dir`, allowlisted).
- **Rendering fixes (2026-10-24, `f58be1c`):** eliminated the stale-content
  flashes when switching threads and views — the reader can no longer show
  the previous thread's messages under a new selection (thread-owned `msgs`
  + an 8-thread instant-revisit cache), the list no longer shows the previous
  label's rows while the new view loads (view-context gate, tick-excluded so
  background mail updates don't flash), a reused message row drops the old
  body as soon as the message id changes, and the reader scroll resets on
  thread change while same-thread refreshes keep the reading position.
  Search typing no longer clears the selection or resets scroll per
  keystroke. Verified in the browser mock (thread→thread, Inbox→Sent,
  cache revisit); tsc + 47 vitest green.
- **Hardening pass (2026-10-24, branch `chore/hardening-ci-docs`):**
  `xlsx` upgraded 0.18.5 → 0.20.3 from the SheetJS CDN (two high advisories,
  GHSA-5pgg-2g8v-p4x9 / CVE-2023-30533 — it parses untrusted attachments);
  CI right-sized to a single Windows job with a `cargo fmt --all --check`
  gate and `pnpm audit --audit-level high`; the release pipeline builds only
  the Windows installer; repository-wide rustfmt drift cleared; M1
  measurement prep (`startup ready` / `window shown` log lines,
  `scripts/measure.ps1`); `PRAGMA optimize` + FTS5 merge after sync passes
  (H3 slice); `docs/VERIFY-LIVE.md` checklist for the live-account session.
  Verified: unread counts are live (`list_labels` computes from the cache,
  not the synced `labels.unread` column) and the store's production paths
  are `Result`-clean (unwraps are all inside `#[cfg(test)]`).

- **Brand & App Icons**: Custom geometric pulse + envelope logo designed and generated across all target resolutions (`32x32`, `128x128`, `256x256`, `512x512`, `.ico`, `.icns`, Android mipmaps, iOS icons). See commit `6555d4b`.
- **On-Demand Attachment Preview**: Click-to-preview modal in Reader supporting images (with zoom/pan), embedded Chromium PDF viewer (`#toolbar=1`), code/text/csv/json with UTF-8 decoding and line-wrap toggle, audio/video players, and fallback binary cards with direct system app launch or download. Full memory cleanup with `URL.revokeObjectURL()`. See ADR 0025.
- **Undo Send (5–10s Grace Window)**: When sending an email, Pulse Mail holds the outgoing message in an in-memory queue with an instant "Undo" button on the UI snackbar before transmitting to Gmail REST. Setting configurable in Settings.
- **Contact Auto-Complete in Compose**: Instant fuzzy typeahead across To, Cc, and Bcc fields querying local SQLite messages and accounts with frequency scoring, colored avatars, keyboard navigation (<kbd>↑</kbd>, <kbd>↓</kbd>, <kbd>Enter</kbd>, <kbd>Tab</kbd>, <kbd>Esc</kbd>), and elevated z-index stacking.
- **Thread Context Menu**: Full right-click and "more" menu on conversation rows: Star / Unstar, Mark as read / unread, Mark Important, Archive, Move to trash, Report spam, Move to tab, and Copy OTP code / subject / thread ID. Styled with gold/yellow active indicators (`text-star`) without jarring blue highlights.
- **Smooth, No-Flash State Updates**: Conversation actions mutate optimistic `overrides` immediately, updating UI state in the background without reloading or flashing the open email reader.
- **Resizable Split Pane**: Draggable splitter divider between the thread list and reader in 2-pane side-by-side mode, with width preferences saved to `localStorage`.
- **Inline Quick Reply**: Bottom of reader now features an inline quick reply bar (Reply / Reply All) directly below the last message.
- **Search Hint & Syntax Autocomplete**: Information icon with syntax documentation tooltip (`from:`, `to:`, `subject:`, `is:`, `has:`, etc.) and real-time operator completion.
- **Gmail Drafts Synchronization**: Fixed draft updates to support Gmail REST `PUT` requirements (`id` parameter) and seamless editing of existing draft messages from the Drafts folder.
- **Tauri v2 IPC Allowlist Enforcement**: Verified capability discipline across `build.rs` and `capabilities/default.json` with automated security tests.
- **Verification**: 132 offline Rust unit tests, 47 UI vitest tests, typecheck, Vite production build, and clippy warning-free.

## Previous handoff (2026-10-22)

- **State:** all work is committed locally (not pushed). Latest commits: 30642cd OTP v2 + category backend · b038852 UI · 406fa6c ADRs 0021/0022 · 51b6e1b + 9704338 OTP tuning (rescan v4) · bbf58da tabs default off (migration 9), send-as name, initials · f3c1da7 docs · 29e11dc send-as name lookup at engine start · this docs commit. Nothing uncommitted.
- **Gates at 29e11dc:** 116 Rust tests, clippy `-D warnings` clean, 38 vitest; build OK at bbf58da.
- **Gmail categories (done):** stored from labelIds and kept fresh by history; all 2,255 cached messages carry labels, so no backfill was needed. Tabs are built but **off by default** (migration 9). See ADR 0021.
- **OTP pipeline (done):** scored detector, corpus of 90 samples at P 1.00 / R 1.00, holdout of 20 at P 1.00 / R 0.93 (the miss is a split alphanumeric `K7Q2-9XMP`). Live rescan v4 on 2,256 messages found 85 codes and links. See ADR 0022.
- **"Inbox not up to date" bug, root cause (fixed):** sync was complete and live; 1,827 INBOX threads match Gmail web. The newer mail is in Updates, Social or Promotions, so the Primary tab stopped at Oct 15. The user's Gmail web shows a single inbox, and the API can't read that setting. Fix: tabs off by default. Mail with no category already counts as Primary.
- **"AG" avatar bug:** the root cause was that the email was passed as the name, and `initials()` split `local@gmail.com` into local + "gmail". Fixed: an address now uses only its local part, and the real `displayName` is preferred. **Still open:** the account's `display_name` is empty. Gmail `settings.sendAs` returns no displayName for this account (log: "Gmail send-as has no display name"), and the only scope is `https://mail.google.com/` (no profile scope). So the avatar shows "A" until a name is set.
- **Next steps:**
  1. Fill the name without a new scope, from the `from_name` of the account's own SENT messages (most common value where `from_email` = the account address), and fall back to the user editing it in Account settings.
  2. Live-check the toast and "Primary + codes" mode on real new mail.
  3. Check "Move to tab" against the Gmail API.
  4. Decide on the Wise privacy-notice link (score 65, possible false positive).
  5. Support split alphanumeric codes.

The matrix below describes `HEAD` plus the working-tree changes listed
above. It describes implemented behavior, not planned features. Plans live in
[ROADMAP.md](ROADMAP.md) and [PLAN.md](PLAN.md).

Legend: **Done** = works end to end on the user's PC · **Partial** = works
but with a real gap · **Stubbed** = visible in the UI but does nothing yet ·
**Not started** = no code · **Untested live** = unit-tested and built, not
yet exercised against the real account.

## Defaults chosen for the user (2026-10-21)

| Topic | Default | Where it can change |
| --- | --- | --- |
| Reading pane | Side pane by default; Full view replaces the list; Below list also available | Settings → Open emails |
| Download to this PC | Per account: 30 days / 90 days / 1 year / everything (full messages); older mail fetched on open; attachments up to 10 MB kept; one 1 GB budget for all accounts | Settings → Accounts → account page; Settings → Storage (`storage_budget_mb`) |
| Accounts | Several accounts, per-account inboxes + "All inboxes"; every account notifies | Settings → Accounts (per-account notifications) |
| Shortcuts | `c` compose, `Shift+C` copy code, `r`/`a`/`f` reply / reply all / forward, Ctrl+K palette | `src/lib/shortcuts.ts` |
| Codes | Not auto-copied (`otp_autocopy=0`); clipboard cleared after 60 s; codes hidden on the lock screen | Settings → Codes |
| Close button | Hides to the tray | Settings → General |
| Google scopes | Gmail plus `openid profile` for account name/photo (approved 2026-10-22); old grants keep working | Reconnect Google in account settings |
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
| History download (H1) | Done (v2, live on the existing account: exact count 2,253 in 1.8 s) | Phases count → index → bodies → files ([ADR 0019](decisions/0019-history-download-v2.md)). Resumable id queue, backoff on rate limits, stall detection with an error and **Retry now**, auto-retry. Progress: "Counting messages… N found", "Downloading n / N · x/s · ~t left", size estimate with and without attachments. Pause/resume | `Engine::history_download`, `hist_call`, `HistSession`, migrations 6–7 |
| Per-account settings page | Done (untested live) | Name, color, Google profile reconnect, notifications, download range, attachments policy, storage, sync, signature, remove. Google photos are cached raster data; custom names override Google | `SettingsPanel.tsx` (`AccountPage`), `set_account_settings`, `account_storage`, `auth/profile.rs` |
| Lazy bodies on open | Done | Older bodies download when opened | `commands::load_thread` |
| Incremental sync (history.list) | Done | Adds, deletes, label changes; history 404 → fresh first sync and re-download of the range | `Engine::delta_sync` |
| IMAP IDLE push | Done | INBOX only, per account; re-IDLE every 20 min; backoff 1 → 60 s | `sync/imap_idle.rs` |
| Polling fallback | Done | 60 s while IMAP is down, 15 min safety poll while up | `sync/mod.rs` |
| Quiet background checks | Done | Push deltas, polls and retries don't show the top bar or change the chip ("Live · now" / "Live · 2m"). They appear only in the chip popover's activity list. One failed background check is logged, not shown; the chip shows an error after two in a row or for sign-in problems. Catch-ups of ≥ 25 messages count as long work | `SyncStatus.foreground`, `LOUD_DELTA`, `lib/sync-view.ts` |
| Top progress bar | Done | Only for user-started or long work (first sync, history download, manual refresh). Appears after 400 ms, stays at least 600 ms | `SyncIndicator.tsx::TopProgress` |
| Rate limits | Done | 429/403 rate reasons retried with backoff; batches paced ≥ 1 s | `gmail.rs` |
| Panic isolation | Done | Sync passes, background job and IMAP loop run under `catch_unwind` | ADR 0007 |
| Labels and counts | Done | Synced from Gmail; sidebar unread counts are computed live from the local cache (`list_labels`), so local actions update them immediately | `Engine::sync_labels`, `store::list_labels` |

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
| Reading pane modes | Done | Side pane / below list / full view (Settings → Open emails). Falls back to full view when the window is too small (side needs ≥ 1024 px wide, bottom ≥ 640 px tall). In full view, j/k open the next/previous conversation | `lib/pane.ts`, `App.tsx` |
| Archive, star, read/unread | Done | Optimistic with rollback; Undo for archive | `Engine::modify_thread` |
| Delete / trash, spam, snooze, label editing | Not started | | |
| Search | Done (untested live) | Local FTS5 first (understands `from:`, `subject:`, `is:unread/read/starred`, `has:attachment`, phrases). 450 ms later Gmail's own search runs on every account with the full query (all Gmail operators: `in:`, `label:`, `before:`, `-x`, `OR`…); missing messages are fetched as metadata and merged by thread. Banner: "Searching all of Gmail…" / "Results from Gmail and this PC". Spam/trash excluded locally | `store/query.rs`, `Engine::server_search`, `lib/search.ts` |
| Compose / reply / reply all / forward | Done (untested live: no real send made) | Docked sheet; From picker; To/Cc/Bcc, subject; **rich editor** (TipTap: bold/italic/underline/strike, lists, quote, links with Ctrl+K, RTL per block, [ADR 0020](decisions/0020-compose-editor.md)); HTML re-sanitized in Rust, text alternative included; attachments by picker or drag-drop (25 MB total, upload endpoint over 3 MB); forward re-attaches the original files; per-account signature. Autosaves to Gmail Drafts; Esc saves and closes. Reply keeps Reply-To/threading. MIME built in Rust (`mail-builder`) | `compose/`, `commands/compose.rs`, `Compose.tsx` |
| Undo send | Done | Send saves the draft, then waits the undo delay (setting `undo_send_secs`) before `drafts.send`; Undo in the snackbar cancels and reopens the draft. Result arrives as a `send-result` event | `commands/compose.rs::send_message`, `cancel_send` |
| Keyboard shortcuts | Done | j/k, o/Enter, u/Esc, /, e, s, Shift+I/U, g+i/s/t/d, Shift+R, `,`, ?, c compose, r/a/f reply / reply all / forward, Ctrl+Enter send, Shift+C copy code, Ctrl+K palette | `lib/shortcuts.ts` |
| Command palette (Ctrl+K) | Done | Fuzzy-ranked: conversation actions, compose/reply, go to folder or label, switch account, layout, theme, settings, shortcuts, "Search mail for…" | `CommandPalette.tsx`, `lib/palette.ts` |

### Notifications and codes

| Feature | State | Notes | Code |
| --- | --- | --- | --- |
| Windows toasts | Partial | New unread INBOX mail < 15 min old, per account; with several accounts the toast says "To <account>". Dev builds show PowerShell as the sender (no AUMID) | `notify/mod.rs` |
| Notification mode per account | Done | All / codes only / off, chosen in onboarding and in Settings → Accounts; global quiet hours in Settings → Notifications | `accounts.notify`, `prefs.quiet_*` |
| OTP extraction (v2) | Done (corpus-tested; untested on live new mail) | Scored pipeline: WebOTP line, multilingual keywords (EN/AR/FR/ES/DE + CJK), split and prefixed codes, Arabic-Indic and full-width digits, HTML emphasis, filters for dates, times, phones, money, URLs, footers and promos, expiry, confidence (store ≥ 50, toast ≥ 60). Magic sign-in links get an "Open" card. "Not a code" and copy feedback per sender. Corpus 90 samples: P/R 1.00/1.00; former holdout 20 now passes as a regression set ([ADR 0022](decisions/0022-otp-pipeline.md)). Cached mail re-scored once at startup | `otp/`, `Reader.tsx` |
| Inbox tabs (Gmail categories) | Done (untested live in the UI) | Primary / Promotions / Social / Updates / Forums from Gmail's `CATEGORY_*` labels, with unread counts. Off by default (Gmail's API can't tell if the user uses tabs; the single inbox matches Gmail with tabs off); per-account switch, "Move to <tab>" in the palette, Important marker, "Primary + codes" notifications ([ADR 0021](decisions/0021-gmail-category-tabs.md)) | `InboxTabs.tsx`, `store::list_threads_in`, `category_counts` |
| Copy code (toast, chip, banner, Shift+C) | Done | Written with the Windows formats that keep it out of Clipboard History, cloud clipboard and clipboard monitors; cleared after 60 s if unchanged (setting); auto-copy off by default (setting) | `secure_clipboard.rs`, `lib.rs::copy_secret` |
| Codes off the lock screen | Done | While the session is locked the toast hides the code | `notify::session_locked` |

### App shell and settings

| Feature | State | Notes |
| --- | --- | --- |
| Tray (Open, Check mail now, Quit) | Done | "Check mail now" checks every account |
| Start with Windows, close = hide, single instance | Done | |
| Settings sheet | Done | General, Appearance, Reading (images, dark mail), Codes, Accounts (range, notifications, history progress, pause, remove), Storage (budget, size used); per-account page, About |
| Theme, density | Done | `localStorage` |
| Sync chip + popover | Done | Per-account history rows with pause/resume; activity list names the account |
| App icon | Done | Full brand icon suite (tray, taskbar, installer, toast) since `6555d4b` |
| Installer (NSIS, per-user) | Partial | Configured, never installed on the PC |
| Tray unread badge | Done | Taskbar overlay with the unread count ("9+" above nine), a dot on the tray icon, per-account tooltip; updates 400 ms after mail changes | `badge.rs`, `tray.rs` |
| Auto-update, code signing | Not started | |

### Engineering

| Item | State | Notes |
| --- | --- | --- |
| Rust unit tests | Done | 133 tests, offline (current working tree) |
| UI unit tests | Done | 47 tests (vitest; current working tree) |
| Clippy / fmt | Done | All-target Clippy with `-D warnings` passes; rustfmt clean and gated in CI |
| Security invariant tests | Done | IPC allowlist in sync, devtools off in release, log calls never format secrets (`security.rs`) |
| Dependency audit | Done | `cargo audit`: no vulnerabilities; 5 allowed warnings, all Linux-only GTK/zbus crates not built on Windows. `pnpm audit`: clean since the `xlsx` 0.20.3 upgrade (2026-10-24) |
| CI | Done | GitHub Actions: one Windows job per PR/push to main running pnpm test, typecheck + build, `pnpm audit`, `cargo fmt --check`, check, test, clippy `-D warnings`; the release pipeline builds only the Windows NSIS installer into a draft release |

## Measured numbers

| What | Result | How |
| --- | --- | --- |
| First rows on a fresh sync, old path | 8.8–20.0 s, 116 messages, ≈6 MB | `sync_bench` |
| First 50 rows, new path | ≈1.0 s | `sync_bench` |
| First 50 rows in the app | 2.4 s (debug build) | log |
| Toast latency, RAM, CPU, cold start | **Not measured** | see [PLAN.md](PLAN.md) |

## Known bugs and limitations

1. **Several paths are unit-tested but untested live** (no real send yet,
   second account, onboarding, server search, reopened drafts). The full
   checklist and measurement session live in
   [VERIFY-LIVE.md](VERIFY-LIVE.md).
2. **Second account untested live.** The multi-account path is unit-tested and the existing account migrated (range 90 days); adding a real second account hasn't been tried.
3. **Dev toasts show PowerShell** as the sender.
4. **IMAP watches INBOX only**; other labels catch up on the 15-min poll.
5. **Cache not encrypted at rest** (relies on the Windows account / BitLocker). Next step: SQLCipher with a DPAPI-protected key (V2), see SECURITY.md.
6. **Account rail and Ctrl+1…9 account switching not built.**
7. **Body eviction** above the storage budget isn't implemented; the download simply stops adding bodies at the budget. (`PRAGMA optimize` + FTS merge run after sync passes; VACUUM and integrity checks are still open, ROADMAP H3.)
