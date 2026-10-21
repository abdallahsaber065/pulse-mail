# 0006: Metadata-first first sync through the Gmail batch endpoint

- Status: accepted (2026-10-21)

## Context
The v0.1 first sync listed 300 messages from 60 days and fetched each as
`format=raw`, 6 at a time, refreshing the UI only at the end. On the user's
account the first rows appeared after 8.8–20 s (about 6 MB). Measured
alternatives (`examples/sync_bench`): 50 single `format=metadata` gets took
3.2 s at 10 in flight and 2.6 s at 25; one batch request with 50 metadata
gets took 0.6 s.

## Decision
- First sync = profile ‖ `messages.list INBOX max 50`, then one
  `POST /batch/gmail/v1` with 50 `messages.get?format=metadata` (From, To,
  Subject, snippet, labels, date). Rows are stored with `body_loaded = 0`.
- Bodies: prefetch `raw` for the newest 12 INBOX messages in the
  background; everything else loads on open (`load_thread`).
- Backfill (90 days, ≤ 500) uses the same batch path, one batch per second
  at most (250 quota units/s). Failed batch parts are retried singly.

## Consequences
- First 50 rows in ≈1 s (benchmark) / 2.4 s (debug app incl. token refresh).
- Opening an old message needs the network once; offline it shows the snippet.
- OTP detection on metadata rows uses subject + snippet only; full bodies
  re-run it when they load.
- A hand-written multipart parser (`parse_batch_response`, unit tested).
