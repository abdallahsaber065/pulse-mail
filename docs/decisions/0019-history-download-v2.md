# 0019: History download v2, attachment policy, one download range

- Status: accepted (2026-10-21, `503baa5`, `f0af3d7`). Amends [0014](0014-full-history-and-storage.md) and [0018](0018-onboarding.md).

## Context
The first history download stalled at "Starting… 500 / 501". Root cause:
the total came from Gmail's rough `resultSizeEstimate` (501), because
`messages_total` was 0 for an existing account. A metadata fallback call
then hit a rate limit (`429`), the background task returned an error, and
`backfill_state` stayed `running` with nothing rescheduling it. The phase was
empty, so the UI showed "Starting…" for good. The real count was 2,253.

There were also two separate ranges that confused the user: a per-account
history range and a global `offline_days` for bodies.

## Decision
- Phases: **count** (`messages.list` 500 per page into a resumable
  `hist_queue` table, giving the exact total and a size estimate with and
  without attachments) → **index** (batch metadata, 50 per call, failed ids
  stay queued) → **bodies** (full messages in the range) → **files**
  (attachments per policy).
- Every Gmail call goes through `hist_call`. A rate limit backs off from 2 s
  up to 60 s and retries while not paused. Network, 5xx and 90 s timeouts
  retry 5 times and then raise `Stalled`. Twelve rounds with no progress
  also raise `Stalled`. On error the account shows the message with
  **Retry now** and retries itself after 30 s · 2ⁿ, at most 15 min.
- Progress shows the phase label, n / total, an EMA-smoothed rate and the ETA.
- **One range per account, "Download to this PC"** (30 days / 90 days /
  1 year / everything). Everything in the range is downloaded in full.
  Older mail is fetched when opened. Global `offline_days` is removed:
  migration 7 deletes the pref and re-queues finished accounts for the
  bodies phase. Only the global storage budget remains, counting the DB plus
  saved attachments. The account page shows its own usage.
- **Attachments per account:** on demand / up to N MB (2, 10 by default, or
  25) / all (50 MB cap per file). Files are stored under
  `files/<account>/<message>/<part>`, with ids restricted to alphanumerics.
  They count against the budget. Everything else is fetched when opened.

## Alternatives
- Keep the estimate-based total: it is wrong by design and can't show
  exact progress.
- Two ranges (index vs bodies): more control, but users found it confusing.
