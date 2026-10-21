# Design spec

Close to Gmail web, so it's familiar on day one, but calmer, denser and faster.
Material 3 shapes and color roles; Windows fonts (Segoe UI Variable).

## Layout

```
┌──────────────────────────────────────────────────────────────────────────┐
│ ☰  [■] Pulse      ( 🔍  Search mail                    ✕ )   ● ⟳ ◐ ⌨ (A) │  64px top bar
├────────────┬─────────────────────────────────────────────────────────────┤
│ ✎ Compose  │ ┌────────────────────────────┬────────────────────────────┐ │
│            │ │ ☆ Sender   [482913] Subj – │ ←                          │ │
│ ▣ Inbox  3 │ │ ☆ Sender   Subject – snip  │ Subject  [Label]           │ │
│ ☆ Starred  │ │ ☆ Sender   Subject – snip  │ ┌ Verification code ─────┐ │ │
│ ▷ Sent     │ │   … virtualized, 40px rows │ │ 4 8 2 9 1 3    [Copy] │ │ │
│ ✎ Drafts   │ │                            │ └────────────────────────┘ │ │
│ ⚠ Spam     │ │                            │ (A) Sender <addr>   date   │ │
│ 🗑 Trash   │ │                            │     sandboxed email body   │ │
│ Labels     │ │                            │                            │ │
│ ⌂ Clients  │ │                            │                            │ │
│ 256px      │ │ list 44% (max 640px)       │ reader (rest)              │ │
└────────────┴─┴────────────────────────────┴────────────────────────────┴─┘
                 main panel: surface, radius 16px, 16px gap to window edge
```

- **Sidebar** 256 px, collapses to a 72 px icon rail (☰). Active item: pill
  `rounded-r-full`, `primary-container` fill, bold.
- **Main panel** sits on the page background with a 16 px radius, like
  Gmail 2022+. The split view is always on at ≥ 1024 px wide; below that the
  reader replaces the list (`u` / Esc to go back).
- **Thread rows** 40 px (Gmail "default" density): star · sender (176 px,
  bold if unread, message count) · OTP chip · **subject** – snippet (faint) ·
  date (right, bold if unread). Unread rows `surface`, read rows `surface-dim`;
  hover gets a raised outline like Gmail; keyboard cursor is a 3 px primary bar
  on the left.
- **Reader:** sticky 48 px action bar, subject 22 px display font, label
  chips, **OTP banner** (green container, 28 px mono code with 0.2 em tracking,
  Copy button) above the messages, 40 px avatar, sender, address, date
  "(N hours ago)", body indented 56 px under the avatar.

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
| `star` | `#f4b400` | `#fdd663` | stars, "polling" status dot |

Contrast: body text and snippets meet WCAG AA on their backgrounds in both themes.

## Typography

- Family: `Segoe UI Variable Text` (UI), `Segoe UI Variable Display`
  (subject, titles), `Cascadia Mono` (codes). Arabic falls back to
  `Noto Sans Arabic` / Segoe UI.
- Sizes: 14 px base; 13 px snippets; 12 px meta and dates; 22 px reader subject; 28 px OTP.
- Weights: 400 regular, 600/700 for unread and active.

## Spacing and shape

4 px grid. Panel radius 16 px, dialogs 24 px, pills full, chips 4-6 px.
Shadows only on hover, raised rows, dialogs and the focused search box.

## Theme

`system` (default), `light`, or `dark`, cycled from the top-bar button and
saved in `localStorage`. The `.dark` class on `<html>` swaps the variables.
Email bodies get a matching text color, but their own colors win, as in Gmail.

## Status indicator (top bar)

- Green dot: "Live: new mail arrives instantly" (IMAP IDLE connected)
- Amber dot: "Checking every minute" (fallback polling)
- Red dot: friendly error text (offline, sign-in expired)
- Grey: not syncing (signed out)

## Keyboard shortcuts (Gmail-compatible subset)

| Keys | Action |
| --- | --- |
| `j` / `k` | next / previous conversation |
| `Enter` / `o` | open |
| `u` / `Esc` | back to list |
| `/` | focus search |
| `c` | copy verification code (Pulse-specific; Gmail uses `c` for compose, which arrives in phase 4 as `Shift+c`) |
| `g` then `i` / `s` / `t` / `d` | Inbox / Starred / Sent / Drafts |
| `Shift+R` | check mail now |
| `?` | shortcut help |

## Motion

Minimal: hover/press color changes only. `prefers-reduced-motion` turns
transitions off. No skeleton shimmer; the cache makes lists appear at once.

## Toasts (Windows)

- Regular mail: title = sender, line 1 = subject, line 2 = snippet (140 chars).
- OTP mail: title = `482913  ·  Sender`, buttons **Copy code** and **Open**,
  long duration (25 s) so there's time to click.
