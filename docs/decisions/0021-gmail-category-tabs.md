# 0021: Gmail's own categories for inbox tabs

- Status: accepted (2026-10-22)

## Context
The user asked whether we can classify mail "using the same Gmail one". We can.
The Gmail API exposes the classification as system labels on every message:
`CATEGORY_PERSONAL` (Primary), `CATEGORY_SOCIAL`, `CATEGORY_PROMOTIONS`,
`CATEGORY_UPDATES`, `CATEGORY_FORUMS`, plus `IMPORTANT` and `STARRED`. A local
classifier would be worse and would disagree with Gmail on the phone and on the web.

## Decision
- **Storage:** none needed. Every message's `labelIds` already goes into
  `message_labels`. `history.list` `labelsAdded`/`labelsRemoved` keep it
  fresh, and the history download (metadata batch) filled it for old mail.
  The user's DB had 2,255 of 2,255 messages labeled (Primary 179, Updates
  1,844, Promotions 159, Social 20, Important 372), so no backfill was needed.
  Migration 8 only adds `accounts.category_tabs`.
- **Tab rule:** a thread goes in the tab of its newest inbox message. Mail
  with no category label counts as Primary, as in Gmail.
- **UI:** tabs above the inbox list (Primary / Promotions / Social /
  Updates / Forums) show unread counts. Empty tabs other than Primary are hidden,
  and narrow lists show icons only. Each account has an "Inbox tabs" switch,
  on by default. The API doesn't expose whether the user turned tabs off on
  the web, so "match Gmail" isn't detectable. Threads carry an Important
  marker. In the palette, "Move to <tab>" uses `modify_thread` to add one
  `CATEGORY_*` label and remove the others. This works within the existing
  `https://mail.google.com/` scope; no new scope.
- **Default off (migration 9, 2026-10-22).** With tabs on, the user saw
  Primary's newest item from Oct 15. Gmail web showed newer mail at the top
  (DigitalOcean, Facebook, Gemini…) and "1–100 of 1,827". The cache was
  complete and live: 1,827 inbox threads, newest at 3:02 AM, and every message
  carried Gmail's labels. Those newer mails are filed under Updates, Social or
  Promotions, so they were correctly not in Primary. The user's Gmail web uses
  a single inbox (tabs off), and the API can't tell us that. Tabs now default
  to off for every account and are one switch away in the account settings.
- **Notifications:** a new per-account mode, "Primary + codes" (`primary`),
  notifies for mail in Primary, and for verification codes from any tab.

## Alternatives
- A local ML or rule classifier: duplicates Gmail and disagrees with it.
- Querying Gmail per tab (`category:promotions`): costs quota on every
  view and doesn't work offline.
