# Design spec

Close to Gmail web, so it's familiar on day one, but calmer, denser and
faster. Material 3 shapes, color roles and motion; Windows fonts (Segoe UI
Variable). This file describes the UI as built (`HEAD`) and, at the end, the
designs for planned layouts.

## Layout (today)

```
▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔▔  3px progress bar (only while syncing)
┌──────────────────────────────────────────────────────────────────────────┐
│ ☰  [■] Pulse    ( 🔍 Search mail          ✕ )   (● Live · just now) ⟳ ◐ ⚙ (A) │  64px top bar
├────────────┬─────────────────────────────────────────────────────────────┤
│ ✎ Compose  │ ┌────────────────────────────┬────────────────────────────┐ │
│ (disabled) │ │ ☆ Sender  [⧉ 482913] Subj –│ ← │ ⬇ ✉ ☆        3 messages │ │  48px action bar
│ ▣ Inbox  3 │ │ ☆ Sender   Subject – snip  │ Subject  [Label]           │ │
│ ☆ Starred  │ │ ★ Sender   Subject – ⬇ ✉   │ ┌ Verification code ─────┐ │ │  hover actions
│ ▷ Sent     │ │   … virtualized rows       │ │ 4 8 2 9 1 3    [Copy] │ │ │
│ ✎ Drafts   │ │                            │ └────────────────────────┘ │ │
│ ⚠ Spam     │ │                            │ (A) Sender <addr>   date   │ │
│ 🗑 Trash   │ │                            │     sandboxed email body   │ │
│ Labels     │ │                            │                            │ │
│ ⌂ Clients 1│ │ list 44% (max 640px)       │ reader (rest)              │ │
└────────────┴─┴────────────────────────────┴────────────────────────────┴─┘
                 main panel: surface, radius 16px, 16px gap to window edge
```

- **Top bar:** menu (collapse sidebar), logo, search pill, then the
  **sync chip**, Check now (spins while syncing), theme cycle, Settings,
  avatar (account menu).
- **Sidebar** 256 px, collapses to a 72 px rail. Active item: pill
  `rounded-r-full`, `primary-container`, bold. Unread counts for Inbox,
  Spam and user labels. Compose is shown but disabled (tooltip).
- **Split view:** right reading pane at ≥ 1024 px; below that the reader
  replaces the list (`u` / Esc to go back). Not configurable yet (see
  [Reading pane modes](#reading-pane-modes)).
- **Thread rows:** star button · sender (176 px, bold if unread, message
  count) · OTP chip · **subject** – snippet · date. Unread rows `surface`,
  read rows `surface-dim`; hover raises the row and swaps the date for
  actions (archive in Inbox, read/unread); keyboard cursor = 3 px primary bar.
- **Density** (Settings): compact 32 px, default 40 px, comfortable 52 px
  (snippet on a second line).
- **Reader:** sticky 48 px action bar (back · archive · read/unread · star ·
  message count), 22 px subject, label chips, OTP banner (green container,
  28 px mono code, Copy → "Copied"), 40 px avatar, sender, date "(N hours
  ago)", body indented 56 px. Messages whose body isn't downloaded yet show
  the snippet and a skeleton.

## Components

| Component | Spec |
| --- | --- |
| **Status chip** (`SyncIndicator`) | 32 px pill: dot + text. Live: green dot with a soft pulse, "Live · just now". Up to date: green, "Up to date · 3 minutes ago". Busy: blue, "Getting your newest mail 20/50" or "Syncing 120/300" (background). Offline: amber, "Offline · showing saved mail". Error: red container, friendly text + **Retry**. Click → popover |
| **Sync popover** | 360 px, `surface-high`, radius 16: icon + headline + connection line ("Live push connected…" / "checking every minute"), progress bars for foreground and background work, "Last checked …" + Check now / Retry now, Activity list (newest first, icon by level, time) |
| **Top progress bar** | 3 px, fixed at the window top. Determinate width for counted work, indeterminate sweep otherwise; background work at 50 % opacity |
| **Dialog** | M3 basic dialog: max 400 px, radius 28, `surface-high`, optional centered icon, 24 px title, 14 px body, text button + filled/danger button. Scrim black 40 %. Enters with fade + scale from 0.92 (220 ms, emphasized decelerate), exits 140 ms. Esc / scrim = dismiss; first (safe) button focused |
| **Snackbar** | Bottom-left, min 288 / max 560 px, `inverse-surface`, radius 6, optional action in `inverse-primary` (Undo, Retry) + close. 4 s, 6 s with an action, 2 s when another is waiting; pauses on hover; slides up 220 ms |
| **Account menu** | Popover from the avatar, 320 px: 72 px avatar with live dot, "Hi, name!", email, "Signed in with Google · token in Windows Credential Manager", then Settings (`,`), Keyboard shortcuts (`?`), Sign out (danger) |
| **Settings sheet** | Right side sheet 420 px, `surface-high`, slides in 260 ms over a 30 % scrim. Sections: General (Start with Windows switch; close behavior segmented), Appearance (theme, density), Notifications (all / codes only / off), About (version, data folder). Changes apply immediately |
| **Segmented button / switch** | M3: full pill, selected segment `secondary-container` with a check; switch 52×32 with a growing thumb |
| **OTP chip** | Pill, `otp-container` text `otp`, mono 12 px, copy icon; on click turns solid green with a check for 1.5 s |
| **Skeletons** | Shimmering bars (1.4 s) in the list while the first page streams in and in the reader while a body loads |
| **Empty states** | 80 px tinted circle with an icon, title, one-line hint: "You're all caught up", "No results for …", "Nothing in Starred" |

## Color tokens

Defined once in `src/styles/app.css` as CSS variables; Tailwind utilities
use them (`bg-surface`, `text-muted`, ...).

| Token | Light | Dark | Use |
| --- | --- | --- | --- |
| `bg` | `#f8fafd` | `#111317` | page behind panels |
| `surface` | `#ffffff` | `#1b1d22` | main panel, unread rows, dialogs |
| `surface-dim` | `#f2f6fc` | `#16181c` | read rows |
| `surface-container` | `#eef2f9` | `#23262c` | chips, code blocks, image notice |
| `search` | `#e9eef6` | `#2a2e35` | search pill |
| `hover` | `#e8edf6` | `#2a2e35` | hover state |
| `selected` | `#c2dbff` | `#0b3a7a` | open thread row |
| `primary` | `#0b57d0` | `#a8c7fa` | buttons, links, focus ring |
| `primary-container` | `#d3e3fd` | `#0842a0` | active nav, Compose |
| `text` | `#1f1f1f` | `#e3e3e3` | body text |
| `muted` | `#444746` | `#c4c7c5` | icons, secondary text |
| `faint` | `#5e5e5e` | `#9aa0a6` | snippets, read dates |
| `outline` / `divider` | `#c4c7c5` / `#e7eaee` | `#5f6368` / `#2c2f35` | borders |
| `otp` / `otp-container` | `#146c2e` / `#c4eed0` | `#6dd58c` / `#0f5223` | verification codes |
| `danger` | `#b3261e` | `#f2b8b5` | errors |
| `star` | `#f4b400` | `#fdd663` | stars, warning dot |
| `surface-high` | `#ffffff` | `#26292f` | dialogs, popovers, settings sheet |
| `secondary` / `secondary-container` | `#4a5f7a` / `#d3e3fd` | `#b6c8e3` / `#2c4467` | dialog icon, selected segment |
| `danger-container` / `on-danger` | `#f9dedc` / `#ffffff` | `#8c1d18` / `#601410` | error chip, destructive button text |
| `inverse-surface` / `inverse-primary` | `#303030` / `#a8c7fa` | `#e3e3e3` / `#0b57d0` | snackbar and its action |
| `skeleton` / `skeleton-hi` | `#e6eaf0` / `#f3f5f9` | `#262a31` / `#30353d` | loading shimmer |

Contrast: body text and snippets meet WCAG AA on their backgrounds in both themes.

## Typography

- Family: `Segoe UI Variable Text` (UI), `Segoe UI Variable Display`
  (subject, titles), `Cascadia Mono` (codes). Arabic falls back to
  `Noto Sans Arabic` / Segoe UI.
- Sizes: 14 px base; 13 px snippets; 12 px meta and dates; 22 px reader subject; 28 px OTP.
- Weights: 400 regular, 600/700 for unread and active.

## Spacing and shape

4 px grid. Panel radius 16, popovers 16, dialogs 28, snackbar 6, pills
full, chips 4–6 px. Shadows on hover, raised rows, popovers, dialogs, sheets
and the focused search box.

## Theme

`system` (default), `light`, or `dark`, cycled from the top bar or set in
Settings, saved in `localStorage`. The `.dark` class on `<html>` swaps the
variables. Email bodies get a matching text color, but their own colors win.

## Motion

Material 3 easing: emphasized decelerate (`cubic-bezier(.05,.7,.1,1)`) for
entering, emphasized accelerate (`cubic-bezier(.3,0,.8,.15)`) for leaving.
Durations 140–260 ms. Used for dialog, popover, sheet, snackbar, fade-in of
the reader and empty states, row color transitions, star press, live-dot
pulse, skeleton shimmer, progress sweep. `prefers-reduced-motion` turns all
animation off.

## Keyboard shortcuts

| Keys | Action |
| --- | --- |
| `j` / `k` | next / previous conversation |
| `Enter` / `o` | open |
| `u` / `Esc` | back to list |
| `/` | focus search |
| `c` | copy verification code (Pulse-specific; Gmail's `c` = compose, planned as `Shift+C`) |
| `e` | archive (Inbox) |
| `s` | star / unstar |
| `Shift+I` / `Shift+U` | mark read / unread |
| `g` then `i` / `s` / `t` / `d` | Inbox / Starred / Sent / Drafts |
| `Shift+R` | check mail now |
| `,` | settings |
| `?` | shortcut help |

Shortcuts are ignored while typing and while a modal dialog/sheet is open.

## Toasts (Windows)

- Regular mail: title = sender, line 1 = subject, line 2 = snippet (140 chars).
- OTP mail: title = `482913  ·  Sender`, buttons **Copy code** and **Open**,
  long duration (25 s).
- Controlled by Settings → Notifications (all / codes only / off).

---

# Planned designs

Not built yet; see [ROADMAP.md](ROADMAP.md) for priorities.

## Reading pane modes

Setting **Reading pane: Right · Bottom · No split** + a top-bar toggle
([ADR 0013](decisions/0013-reading-pane-modes.md)).

```
Right (today)                 Bottom                        No split (Gmail default)
┌────────┬──────────┐         ┌───────────────────┐         list ──open──▶ ┌───────────────────┐
│ list   │ reader   │         │ list              │                        │ ← ⬇ ✉ ☆   ‹ 3/50 › │
│        │          │         ├───────────────────┤                        │ reader, full width │
│        │          │         │ reader            │         ◀──u / Esc──── │ j/k = next/prev    │
└────────┴──────────┘         └───────────────────┘                        └───────────────────┘
 divider draggable             divider draggable             list scroll + cursor restored on back
```

- Shift+Enter or middle-click: open the thread in its own window (same
  reader, no sidebar, 900×760).
- Narrow windows (< 1024 px) fall back to No split in any mode.

## Multi-account

```
┌──┬──────────┬────────────────────────────────────┐
│(A)│ ✎ Compose│ ● ▌ Sender   Subject – snippet      │  ▌ = account color stripe
│ 3 │ ▣ Inbox  │ ● ▌ Sender   Subject – snippet      │      (unified view only)
│(W)│ ☆ Starred│                                    │
│ 1 │ …        │                                    │
│(+)│          │                                    │
│ ∑ │          │                                    │
└──┴──────────┴────────────────────────────────────┘
 56px account rail: avatars with color ring + unread badge,
 ∑ = unified inbox (Ctrl+0), Ctrl+1…9 = accounts, + = add account
```

- The status chip shows the selected account; the popover lists every account.
- Toasts from non-selected accounts carry the account's color/name; a click
  switches to that account and opens the thread.
- Compose has a From picker (account + aliases).

## Command palette (Ctrl+K)

Centered 560 px dialog, search field, grouped results (Go to · Actions ·
Accounts · Contacts · Recent), keyboard only, Enter runs, Esc closes.

## Compose

Docked sheet bottom-right (520×560, minimizable, up to 3 stacked) or
pop-out window; inline reply at the end of a thread. Toolbar: bold, italic,
lists, link, quote, attach. Footer: Send (Ctrl+Enter) with a dropdown for
"Send later", undo-send snackbar.

## Settings layout (target)

The side sheet grows into a full settings page with a left nav when there
are more than ~5 sections: General · Accounts · Reading · Appearance ·
Notifications · Codes · Storage · Privacy · Advanced. Full option list in
[ROADMAP U3](ROADMAP.md#u3).
