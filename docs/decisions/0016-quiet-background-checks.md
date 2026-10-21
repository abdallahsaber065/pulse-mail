# 0016: Background checks are quiet; progress UI is for long or user-started work

- Status: accepted (2026-10-21, `1e2bfdc`)

## Context
Every IMAP push, the 15-minute poll and each retry used to flash the top
progress bar and change the chip text ("Checking for mail…"). With push
mail that happens many times an hour and reads as the app being busy or
broken.

## Decision
- `SyncStatus.foreground` is true only for the first sync, a manual refresh
  (`sync_now(manual = true)`: Check now, Shift+R, tray), the history
  download, or a catch-up of at least 25 messages (`LOUD_DELTA`).
- Background passes never change the chip; it stays "Live · now" /
  "Live · 2m" (`shortAgo`). They are listed only in the chip popover's
  activity log.
- Errors: one failed background check is logged only. The chip shows an
  error after two consecutive failures or immediately for sign-in problems
  (`invalid_grant`). User-started work shows its errors at once.
- The top bar appears only after 400 ms of foreground work and stays at
  least 600 ms, so quick work never flashes it.

## Consequences
The chip no longer proves that push works on every message; the popover
does (connection line and activity list). A stalled background loop is
visible only through the "Live · 15m" age growing.
