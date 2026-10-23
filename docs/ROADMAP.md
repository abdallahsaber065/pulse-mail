# Roadmap: feature catalog

Everything Pulse Mail could do next, grouped by area and prioritized.
What already works is in [STATUS.md](STATUS.md); phases and acceptance
criteria are in [PLAN.md](PLAN.md). This file is the backlog those phases
pull from.

**Priority**
- **P0**: next up. Needed before Pulse Mail fully replaces Gmail web for daily use.
- **P1**: soon after. Big quality or convenience wins.
- **P2**: nice to have.
- **Later**: ideas worth keeping; no commitment.

**Origin**: 🧑 = asked for by the user · 💡 = suggestion, open for discussion.

Each item: what it is → design notes → Gmail API / IMAP implications.
Quota numbers: Gmail allows 250 units per user per second; `messages.get` = 5,
`messages.list` / `history.list` / `labels.list` = 5 / 2 / 1,
`messages.send` = 100, `messages.modify` / `threads.modify` = 5 / 10.

## Recommended order

| # | Item | Pri | Origin |
| --- | --- | --- | --- |
| 1 | [Measure toasts and RAM, add `toast shown` log](#m1) | P0 | 💡 |
| 2 | ~~[Infinite scroll + body-load error state](#r3)~~ ✅ `66dcd8a` | P0 | 💡 |
| 3 | ~~[Full mailbox history, resumable](#h1)~~ ✅ `38b1870` | P0 | 🧑 |
| 4 | [Server search merged with local FTS](#s1) | P0 | 💡 |
| 5 | ~~[Multi-account, all syncing at once](#a1)~~ ✅ `38b1870` (+ [onboarding](#a5)) | P0 | 🧑 |
| 6 | [Reading pane modes (right, bottom, full page, window)](#r1) | P0 | 🧑 |
| 7 | [Reply / forward / compose with drafts](#c1) | P0 | 💡 |
| 8 | ~~[Attachments + inline images](#c4)~~ ✅ `15529c1` | P0 | 💡 |
| 9 | [App icon, AUMID, installer, tray badge](#w1) | P1 | 💡 |
| 10 | [Command palette (Ctrl+K)](#k1) | P1 | 💡 |

### Current priorities (2026-10-24)

Verification and hardening landed on `chore/hardening-ci-docs` (xlsx 0.20.3,
Windows-only CI + fmt/audit gates, M1 markers + measure script, H3 optimize
slice, VERIFY-LIVE.md). Before new features: run the live session — it is the
only thing standing between "unit-tested" and "known to work" for send,
second account, toasts and the performance table.

| Order | Feature | Acceptance and boundary |
| --- | --- | --- |
| 0 | **Live verification + measurement session** | Work through [VERIFY-LIVE.md](VERIFY-LIVE.md) on the real PC (one or two sessions). Fills the PLAN.md Measured column, decides ADR 0004, and unblocks everything below. No code. |
| 1 | ~~**Sleep/resume + network-change reconnect (X2)**~~ ✅ built 2026-10-24 (ADR 0026): clock-jump detector triggers a quiet catch-up on all accounts after wake. Open: nudge the IMAP re-handshake itself (still on backoff) and network-change events beyond the webview's online/offline. |
| 2 | ~~**Crash logs (V4)**~~ ✅ built 2026-10-24 (ADR 0026): rolling daily logs, 7-day retention, panic hook, "Open logs folder" in Settings. |
| 3 | **Reopen existing Gmail drafts** | Open from Drafts into compose with recipients, rich text and attachments intact; save back to the same draft. v0.2.0 built the PUT path; VERIFY-LIVE §1 exercises it. |
| 4 | ~~**Quiet hours (N2)**~~ ✅ built 2026-10-24 (ADR 0026): local window crossing midnight, OTP exception, Settings UI. Live check in VERIFY-LIVE §3. |
| 5 | **Trash / spam / archive-everywhere + label picker (T1/T2)** | `#`, `!`, `l` via `threads.modify`/`labels.*`, undo snackbar for each. Completes Phase 2 triage. |
| 6 | **Offline outbox (X1)** | Durable retry/idempotency design first (ADR); queue label changes and sends while offline, replay in order. |

Deferred until 0–5 are done: notification rules (N1), bulk archive
(T0 extension), unsubscribe helper (T7), send later (C2), and the
engineering slices below. Performance measurement stays open until the
session runs; no live-account work without the user.

**Standing engineering slices** (pick up opportunistically, none block
features): split `store/mod.rs` (1,900 lines) and `App.tsx` (1,500 lines)
into modules; 3–5 Playwright smoke flows against the browser mock (M3,
catches IPC-contract drift the unit tests can't); VACUUM + integrity check
(H3 remainder); body eviction above the storage budget (ADR 0014).

---

## H. History and storage

<a id="h1"></a>

### H1. Full mailbox history 🧑 · P0 · ✅ built (`38b1870`)
> As built: state on the `accounts` row instead of `backfill_jobs`, one
> scope per account (the onboarding range), pause/resume, bodies within
> the per-account range up to the budget; v2 adds the exact count, a resumable queue, stall detection,
> and an attachments policy. Eviction and per-label overrides are still open. See
> [ADR 0014](decisions/0014-full-history-and-storage.md) and [ADR 0019](decisions/0019-history-download-v2.md).

Fetch *all* mail, not just 90 days / 500 messages, without slowing live mail.
- **Design:** a resumable job per account and per label. Walk
  `messages.list` pages newest → oldest; persist `pageToken` and counters in a
  `backfill_jobs(account_id, scope, page_token, done, total_estimate, state)`
  table so it resumes after restarts and sleep. Fetch metadata through the
  existing batch path (50 per call, ≥ 1 s apart), bodies only on open or for
  "keep offline" labels. Live sync and user actions always run first: the job
  yields whenever the engine is busy. Progress goes to the existing
  `backfill` status ("Syncing 12,400 / 48,000") and the activity log.
- **UI:** Settings → Storage: "Keep on this PC: last 90 days / 1 year / all",
  per-label overrides, pause/resume, size used, "free up space".
- **Storage limits / eviction:** metadata for everything is cheap (≈1 KB a
  message, 50k messages ≈ 50 MB). Bodies are the big part: keep bodies for
  the last N days + starred + "keep offline" labels; evict others LRU by
  `last_opened_at` when over a size budget (default 1 GB). Evicted rows keep
  metadata and FTS on subject/sender/snippet.
- **Attachments policy:** never prefetch; download on demand to a cache
  folder with its own budget; images under 200 KB may be cached for inline display.
- **Gmail:** `messages.list` (5 units/page of 500 ids) + batched
  `messages.get?format=metadata`. 50k messages ≈ 1,000 batches ≈ 17 min at
  the quota-safe pace. `labels.get` gives `messagesTotal` for the estimate.
  IMAP not involved.

<a id="h2"></a>

### H2. Older mail on demand 💡 · P1
"Load older" at the end of a list, and in search results ("Search all mail
on Gmail"). Fetches the next `messages.list` page for that label or query
only. Cheap stepping stone before H1 is complete.

<a id="h3"></a>

### H3. Database maintenance 💡 · P2 · partially built (2026-10-24)
> As built: `PRAGMA optimize` + FTS5 `merge` run after every delta-sync pass
> that changed something (cheap, non-fatal). Still open: `VACUUM` after big
> evictions, integrity check on startup after a crash, size shown in Settings.
`PRAGMA optimize` on idle, `VACUUM` after big evictions, FTS `merge`,
integrity check on startup after a crash, size shown in Settings.

## A. Accounts

<a id="a1"></a>

### A1. Multi-account, all live at once 🧑 · P0 · ✅ built (`38b1870`)
> As built: account switcher in the account menu (rail and Ctrl+1…9 still
> open), every account notifies, toasts name the account.

N Gmail accounts, each syncing all the time, notifying even when not selected.
- **Design:** one `AccountRuntime` per account (its own `TokenManager`,
  engine task, IMAP IDLE task, history cursor, status cell and backfill job)
  in an `AccountRegistry` keyed by `account_id`. Shared: one SQLite store,
  one HTTP client (HTTP/2 connection reuse), one notifier. Every IPC command
  takes `account_id` (or `"all"` for the unified inbox). Events carry
  `account_id`. Details in [ARCHITECTURE.md](ARCHITECTURE.md#multi-account-target-architecture)
  and [ADR 0012](decisions/0012-multi-account.md).
- **UI:** an account rail (left edge, avatars with per-account color ring and
  unread badge), Ctrl+1…9 to switch, Ctrl+0 for the unified inbox, account
  chip on each row in the unified view, account picker in compose.
  Switching is instant because every account is already cached and synced.
- **Notifications:** toasts from every account; title prefixed with the
  account color dot / short name when more than one account exists; click
  switches to that account and opens the thread. Per-account rules (N1).
- **Budgets:** each account costs one IMAP socket and a few KB of state.
  Cap concurrent backfills to one at a time across accounts; live delta
  syncs run in parallel. Per-account quota is separate at Google, so
  accounts don't slow each other.
- **Gmail / IMAP:** nothing new per account; N IMAP IDLE connections
  (Gmail allows 15 per account). Keyring entry per account already uses
  `refresh-token:<email>`.

<a id="a2"></a>

### A2. Unified inbox 💡 · P1 · ✅ built (`38b1870`, account stripe still open)
All accounts' INBOX merged by date, with an account color stripe on each
row. One SQL query across `account_id`s; actions route to the right account.

<a id="a3"></a>

### A3. Per-account appearance 💡 · P1
> Built in the current working tree (2026-10-22; untested live): Google name
> and locally cached profile photo on sign-in/reconnect; manual names take
> priority (ADR 0023). Name/color settings already exist.
Color (auto from email hash, editable), display name, optional emoji/avatar.

<a id="a5"></a>

### A5. Add-account onboarding 🧑 · P0 · ✅ built (`38b1870`)
First run and "Add account": welcome → Google sign-in → download range
(30 days / 90 days / 1 year / everything) with size estimates from
`messagesTotal` / `resultSizeEstimate`, offline copies and storage budget,
attachments policy → notifications (all / codes only / off) → live
progress (n/total, phase, speed, ETA, pause/resume, continue to inbox;
resumes after restart) → done. History progress also in Settings →
Accounts and the chip popover. [ADR 0018](decisions/0018-onboarding.md).

<a id="n0"></a>

### N0. Quiet background checks 🧑 · P0 · ✅ built (`1e2bfdc`)
Push, poll and retry passes don't show the top bar or change the chip;
the bar is only for user-started or long work (after 400 ms, ≥ 600 ms).
[ADR 0016](decisions/0016-quiet-background-checks.md).

<a id="a4"></a>

### A4. Send-as aliases 💡 · P2
`users.settings.sendAs.list` for "From" options in compose.

## R. Reading and layout

<a id="r1"></a>

### R1. Reading pane modes 🧑 · P0 · ✅ built (`a26a03b`, right/bottom/full; separate window open)
> Working-tree refinement: Settings → **Open emails** is now first, with
> Side pane / Full view / Below list and explanatory text. The saved choice
> applies immediately. Resizable dividers and separate windows remain open.
Like Gmail's "Reading pane" setting:
- **Right split** (today's behavior, resizable divider, width remembered).
- **Bottom split** (list on top, reader below; good on portrait screens).
- **No split / full page:** the reader replaces the list in place, with
  back, older/newer arrows (`k`/`j` move between threads while reading) and
  the list scroll position restored on return. Gmail's default.
- **Open in new window** (Shift+Enter or middle-click): a second webview
  window for one thread; closes independently.
- **Design:** a `layout` preference (`right | bottom | none`) in Settings and a
  quick toggle in the top bar; the list and reader components stay the same,
  only the container changes. See [ADR 0013](decisions/0013-reading-pane-modes.md)
  and [DESIGN.md](DESIGN.md#reading-pane-modes).
- **Gmail:** none.

<a id="r2"></a>

### R2. Conversation vs message view 💡 · P1
Setting: group by thread (today) or list individual messages. In
conversation view: collapse older read messages to one-line headers, expand
on click, "Expand all"; quoted text folded ("…"). Message view uses the
existing `messages` rows directly.

<a id="r3"></a>

### R3. List paging and reader robustness 💡 · P0
Infinite scroll (fetch the next 200 rows from SQLite as the virtual list
nears the end, then H2 for Gmail). Fix the reader so a failed body download
shows an inline "Couldn't load this message · Retry" instead of a skeleton
forever (STATUS bug 11).

<a id="r4"></a>

### R4. Print and export 💡 · P2
Print a thread (clean print stylesheet), export a message as `.eml`
(`format=raw` is already what we fetch), "Show original".

<a id="r5"></a>

### R5. Drag and drop 💡 · P2
Drag a row onto a sidebar label to label/move it; drag attachments out to
Explorer; drop files on compose to attach.

<a id="r6"></a>

### R6. Mini window 💡 · Later
A compact always-on-top window (inbox + codes only), toggled from the tray.

## S. Search

<a id="s1"></a>

### S1. Search: Gmail operators + server search 💡 · P0 · ✅ built (`a5ee937`, chips/advanced filter open)
- **Design:** parse Gmail operators (`from:`, `to:`, `subject:`,
  `has:attachment`, `is:unread`, `label:`, `before:`/`after:`, `newer_than:`)
  into SQL filters for the local cache; at the same time call
  `messages.list?q=<query>` and merge results (dedupe by id, fetch missing
  metadata in one batch). Local results show instantly; server results stream
  in with a "Searching Gmail…" row. Group results by thread.
- **UI:** search chips/suggestions (recent searches, contacts, labels), a
  filter dropdown like Gmail's advanced search.
- **Gmail:** `messages.list` 5 units per page; nothing else.

<a id="s2"></a>

### S2. Saved searches 💡 · P2
Pin a query to the sidebar like a label.

## C. Compose and attachments

<a id="c1"></a>

### C1. Compose, reply, forward 💡 · P0 · ✅ built (`feee336`, plain text; rich text open)
- **Design:** compose as a docked sheet (bottom right, like Gmail) or full
  window; reply inline at the bottom of a thread. Rich text with a small
  editor (bold, italic, lists, links, quote) emitting simple HTML + plain-text
  alternative; build MIME in Rust (`mail-builder`), keep `In-Reply-To` /
  `References` and `threadId` so Gmail threads it. Signature per account.
- **Drafts:** autosave every few seconds to `users.drafts.create/update`
  (stored locally first so nothing is lost offline). Drafts label syncs back.
- **Gmail:** `messages.send` (100 units) / `drafts.send`. No new scope
  (full mail scope already granted). Gmail shortcut `c` conflicts with our
  copy-code `c`: compose becomes Shift+C (or make it configurable).

<a id="c2"></a>

### C2. Undo send and send later 💡 · P1 · undo send ✅ (`feee336`); send later open
Undo send: hold the message locally for 5–30 s (setting) before calling
`messages.send`. Send later: keep a local outbox with a send time; the core
sends it even if the window is closed (the app is always on). Gmail's own
scheduled send isn't exposed in the API, so this is client-side; the PC must
be on at that time (warn in the UI).

<a id="c3"></a>

### C3. Contacts autocomplete 💡 · P1 · ✅ done (v0.2.0)
Typeahead auto-complete across To/Cc/Bcc backed by SQLite message/account frequency
ranking (`search_contacts` IPC). Keyboard navigable, colored avatars, and elevated stacking.

<a id="c4"></a>

### C4. Attachments & In-App Preview 💡 · P0 · ✅ done (v0.2.0)
List attachments under each message (name, size, type icon); click-to-preview popup
for images (with zoom), PDFs (embedded Chromium viewer), code/text/csv/json (UTF-8 monospace
viewer with line wrap), and audio/video. Streaming `get_attachment_data` IPC without temp files
and automatic URL revocation. Direct "Open in App" and "Download" buttons.

<a id="c5"></a>

### C5. Quick reply from the toast 💡 · P2
WinRT toasts support a text input: "Reply" box on the toast sends a plain
reply through C1's send path.

## T. Triage and organization

<a id="t1"></a>

### T0. Select conversations and bulk mark-read 🧑 · P0
> Built in the working tree (2026-10-22): row checkboxes, Select all matching
> cached conversations (including unloaded rows), unchecked exceptions,
> frozen account/thread keys in Rust, bounded read batches, Stop and failure
> reporting. Gmail-only searches select loaded results explicitly. List
> rendering stays virtualized; folder/local-search paging grows in 200-row
> chunks. Shift+I acts on the selection. [ADR 0024](decisions/0024-bulk-selection.md).

### T1. Trash, spam, mute 💡 · P1
Delete (`#`), report spam (`!`), mute (`m`) via `threads.modify`
(TRASH / SPAM labels; mute = custom label + rule). Undo snackbar for all.
Archive everywhere, not only in INBOX (remove the current label).

<a id="t2"></a>

### T2. Labels management + colors 💡 · P1
Create, rename, color, nest, delete labels (`labels.create/patch/delete`);
apply labels from a picker (`l`) and move (`v`). Show Gmail label colors.

<a id="t3"></a>

### T3. Snooze 💡 · P1
Gmail's snooze isn't in the public API. Client-side: archive + store
`snooze_until`; the always-on core moves it back to INBOX (and marks unread)
at that time. Shows in a local "Snoozed" view. Works only while the PC runs;
catch up on next start.

<a id="t4"></a>

### T4. Categories tabs 💡 · P2 · ✅ built (2026-10-22), off by default (migration 9); open: live "Move to tab" check
> As built: tabs with unread counts, a per-account switch, "Move to tab",
> an Important marker and the "Primary + codes" notification mode. See
> [ADR 0021](decisions/0021-gmail-category-tabs.md).

Primary / Promotions / Social / Updates / Forums tabs from
`CATEGORY_*` labels (already synced); notify only for Primary (setting).

<a id="t5"></a>

### T5. Important / priority inbox 💡 · P2
Use Gmail's `IMPORTANT` label; "Important first" section and a notification
rule "only important".

<a id="t6"></a>

### T6. Filters view 💡 · Later
Read-only list of Gmail filters (`settings.filters.list`), later create
"filter messages like this" (needs `gmail.settings.basic`, so ask first).

<a id="t7"></a>

### T7. Unsubscribe helper 💡 · P2
Detect `List-Unsubscribe` / `List-Unsubscribe-Post` headers; one button that
does the one-click POST (RFC 8058) or opens the mailto/https link. Add
`List-Unsubscribe` to the metadata headers we request.

## N. Notifications

<a id="n1"></a>

### N1. Notification rules 💡 · P1
Per account and per sender/label: always / codes only / never; VIP senders
always notify; mute labels; "only Primary".

<a id="n2"></a>

### N2. Quiet hours and Focus 💡 · P1
Quiet hours schedule; respect Windows Do Not Disturb / Focus (toasts are
queued by Windows anyway; we also suppress sound). OTP toasts can bypass
quiet hours (setting) because they're time-critical.

<a id="n3"></a>

### N3. Tray badge and unread count 💡 · P1
Tray icon overlay with the INBOX unread count (all accounts), taskbar
overlay badge when the window exists; tooltip "3 unread · Live".

<a id="n4"></a>

### N4. Grouped toasts 💡 · P2
When 5+ mails arrive together, one summary toast instead of five.

## O. Verification codes

<a id="o1"></a>

### O1. OTP extras 💡 · P1 · mostly built (corpus 90 and former holdout 20 pass; split alphanumeric codes in current working tree; live toast check open)
> As built (2026-10-22): auto-copy, clipboard clear, lock-screen hiding,
> per-sender learning ("Not a code" and copy feedback), magic-link detection
> with "Open" in the reader, expiry on the toast. The scored pipeline is in
> [ADR 0022](decisions/0022-otp-pipeline.md). Still open: SPF/DKIM checks
> for first-time senders, expiring chips, "Open link" on the toast, and
> spaced alphanumeric codes. Hyphenated mixed codes are supported in the
> current working tree (2026-10-22).

- Auto-copy new codes (off by default) with a toast "Code copied".
- Clear the clipboard after N seconds if it still holds the code (default 60 s).
- Hide/expire code chips after N minutes (codes are short-lived).
- "Hide codes on the lock screen": toast without the code when the
  session is locked.
- Sender trust: learn senders that send codes; lower the threshold for them,
  never show a code from a first-time sender that fails SPF/DKIM
  (`Authentication-Results` header).
- Magic-link detection ("Sign in" buttons) with "Open link" on the toast.

## V. Privacy and security

<a id="v1"></a>

### V1. Remote images per sender 💡 · P1
"Always show images from this sender" allowlist (local table), plus a
global setting. Optional image proxying isn't possible without a server;
keep blocking by default.

<a id="v2"></a>

### V2. Encryption at rest 💡 · P2
Options: SQLCipher (bundled, adds ≈1–2 MB) with a key stored in Credential
Manager / DPAPI; or DPAPI-encrypt only bodies. Trade-off: FTS on encrypted
SQLCipher works; DPAPI-per-field breaks FTS on bodies.

<a id="v3"></a>

### V3. App lock 💡 · P2
Lock the window with Windows Hello (`UserConsentVerifier`) after idle or on
demand; toasts keep working but without content while locked.

<a id="v4"></a>

### V4. Crash logs (local only) 💡 · P1
Rolling log file in `%LOCALAPPDATA%\com.abdallah.pulsemail\logs` (size-capped,
no message content), panic hook writes the backtrace there, "Open logs folder"
in Settings.

## W. Windows integration

<a id="w1"></a>

### W1. Identity: icon, AUMID, installer 💡 · P1 · tray/taskbar unread badge ✅ (`79cfd04`)
Real Pulse icon set (tray light/dark variants, unread overlay); register
an AppUserModelID + Start-menu shortcut so toasts show "Pulse Mail" in dev
too (or test only with the installed build); NSIS installer tested on the
PC; COM activator so Action Center clicks work after a restart.

<a id="w2"></a>

### W2. Auto-update and code signing 💡 · P2
Tauri updater with a signed manifest (local file or GitHub release, if a
repo ever exists); code signing (Azure Trusted Signing or a cert) to avoid
SmartScreen. Ask before adding any update endpoint.

<a id="w3"></a>

### W3. Global hotkey 💡 · P1
Configurable (default Ctrl+Alt+M) to show/hide the window; another to copy
the latest code without opening anything.

<a id="w4"></a>

### W4. `mailto:` handler and deep links 💡 · P2
Register as a `mailto:` handler (opens compose), `pulsemail://thread/<id>` links.

## K. Keyboard and speed

<a id="k1"></a>

### K1. Command palette 💡 · P1 · ✅ built (`31f6643`, contacts/recents open)
Ctrl+K: jump to label/account, run any action (archive, snooze, settings),
search contacts, "copy last code". Fuzzy match, recent commands first.

<a id="k2"></a>

### K2. Full Gmail shortcut set 💡 · P2
`x` select, `*a`/`*n` select all/none, bulk actions, `[`/`]` archive and
move, `#`, `!`, `l`, `v`, `r`/`a`/`f` reply/all/forward, `Shift+C`
compose; editable keymap.

## U. Look and feel

<a id="u1"></a>

### U1. Themes and accent 💡 · P2
Accent color picker (Material 3 dynamic color from one seed), high-contrast
theme, follow Windows accent color.

<a id="u2"></a>

### U2. Accessibility, RTL and i18n 💡 · P1
Screen reader labels everywhere (most exist), focus order, visible focus.
RTL: Arabic subjects and bodies render with `dir="auto"`; full RTL UI
mirror when the UI language is Arabic. i18n: extract strings, English +
Arabic.

<a id="u3"></a>

### U3. Settings catalog 🧑 · P0 (grow with each feature)
All options grouped (✓ = exists today):
- **General:** start with Windows ✓, close button (tray/quit) ✓, global
  hotkey, default account, language.
- **Accounts:** add/remove, color, display name, signature, sync scope
  (90 days/1 year/all), keep-offline labels, per-account notifications.
- **Reading:** reading pane (right/bottom/none), conversation view on/off,
  mark as read (immediately / after N s / never), show images (ask/always/
  per sender), default reply (reply/reply all), undo send delay.
- **Appearance:** theme ✓, density ✓, accent, font size, show snippets,
  show avatars/sender images.
- **Notifications:** all/codes only/off ✓, per account, VIP, quiet hours,
  sound, lock-screen privacy, grouped toasts.
- **Codes:** auto-copy, clear clipboard after, hide after N min.
- **Storage:** size used, body budget, attachment cache, free space, rebuild search index.
- **Privacy:** app lock, encryption at rest, clear cache.
- **Advanced:** log level, open logs, activity history, about ✓.

## X. Reliability

<a id="x1"></a>

### X1. Offline mode + outbox 💡 · P1
Queue every action (label changes, sends, drafts) in a local `outbox`
table when offline; replay in order when back; show "Waiting to send (2)".
Today's optimistic actions roll back when offline instead.

<a id="x2"></a>

### X2. Sleep/resume and network change 💡 · P1
Listen for power resume and network change events; reconnect IMAP and run a
delta immediately instead of waiting for backoff.

## M. Measurement and quality

<a id="m1"></a>

### M1. Measure what matters 💡 · P0
Add log lines with timestamps for `toast shown`, window first paint, cold
start; a tiny script to sum RAM of `pulse-mail` + its WebView2 children;
record results in PLAN.md. Decide ADR 0004 from the numbers.

<a id="m2"></a>

### M2. Local performance telemetry 💡 · P2
An in-app "Diagnostics" page: last sync durations, quota errors, IMAP
uptime, RAM. Local only, never sent anywhere.

<a id="m3"></a>

### M3. Tests and CI 💡 · P1
Component tests for the list/reader (Solid testing library), Playwright
against the browser mock, recorded Gmail fixtures for sync tests, GitHub
Actions (or a local pre-commit script) running fmt/clippy/tests/build.
