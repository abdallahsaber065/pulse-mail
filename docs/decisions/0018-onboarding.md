# 0018: Multi-step onboarding that chooses the download range up front

- Status: accepted (2026-10-21, `38b1870`)

## Context
The user wants a polished first run and "Add account" flow: choose how much
mail to download, see what it costs, choose notifications, and watch the
download, without waiting for it before reading mail.

## Decision
- Steps: welcome → Google sign-in → range (30 days / 90 days / 1 year /
  everything) → notifications (all / codes only / off) → live progress →
  done. The same component serves first run and "Add another account".
- Size estimates come from `users.getProfile.messagesTotal` (everything)
  and `messages.list?q=newer_than:…` `resultSizeEstimate` (one call per
  range), at about 3 KB metadata per message plus 45 KB per offline body.
  The range step also shows the offline-copies window and the storage budget,
  and says attachments are never pre-downloaded.
- Sign-in adds the account in state `setup` and starts live sync at once, so
  the inbox fills while the user is still choosing. `finish_onboarding`
  stores the range and notification mode and moves the account to
  `pending`; the history download then starts. Notifications stay off
  until `finish_onboarding`.
- "Continue to inbox" leaves the progress step; the download keeps going and
  stays visible in the chip popover and Settings → Accounts. Pause/resume is
  per account and resumes from the saved page after a restart.
- An account still in `setup` reopens the onboarding at the range step on
  the next launch.

## Alternatives
- Download first, then show the inbox: slow first impression. Rejected.
- Range in Settings only: users wouldn't find it before the default ran.
