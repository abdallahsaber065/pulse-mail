# 0014: Full-history backfill with a storage budget

- Status: proposed (2026-10-21)

## Context
Backfill stops at 90 days / 500 messages, so older mail can't be read or
searched. Downloading everything with bodies could take gigabytes.

## Decision (proposed)
- Metadata for the whole mailbox, fetched by a resumable job per account and
  scope (`backfill_jobs` table with the `pageToken`), newest first, batch
  path at ≤ 250 units/s, always yielding to live sync and user actions.
- Bodies only for: recent mail (setting: 90 days by default), starred,
  "keep offline" labels, and anything opened. LRU eviction of other bodies
  over a size budget (default 1 GB); metadata and subject/sender/snippet
  FTS stay.
- Attachments on demand only, separate cache budget.
- Details: [ROADMAP H1](../ROADMAP.md#h1).

## Consequences
50k messages ≈ 50 MB of metadata and ≈ 17 min of background work. Search
over evicted bodies falls back to Gmail server search ([ROADMAP S1](../ROADMAP.md#s1)).
