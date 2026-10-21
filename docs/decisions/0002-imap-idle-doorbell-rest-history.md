# 0002: IMAP IDLE as the push doorbell, Gmail REST history for data

- Status: accepted (2026-10-21)

## Context
Near-real-time notifications are required without running a server.
Gmail `users.watch` pushes through Cloud Pub/Sub (needs a GCP topic and a
subscriber, plus a renewal every 7 days). IMAP IDLE is push over one TLS socket.
Downloading and threading mail over IMAP is complex; the REST API gives JSON,
labels, thread ids and history diffs.

## Decision
- Hold one IMAP connection (XOAUTH2) in IDLE on INBOX, only as a wake-up signal.
- On wake, call `users.history.list` from the saved historyId, fetch new
  messages with `messages.get?format=raw`.
- Fallbacks: polling every 60 s while IMAP is down, every 15 min as a safety
  net while it's up; full resync when the historyId is too old (404).

## Consequences
- Push latency of about 1-2 s with no cloud setup beyond the OAuth client.
- Needs the full `https://mail.google.com/` scope (IMAP requires it).
- Only INBOX changes ring the doorbell; other labels catch up on the next sync.
- Later option: add Pub/Sub *pull* (no public endpoint) as a second source if IDLE proves flaky.
