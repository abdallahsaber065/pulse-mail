# 0012: Multi-account with one runtime per account

- Status: proposed (2026-10-21)

## Context
The user wants several Gmail accounts working at the same time, with
notifications from accounts that aren't selected and instant switching.
Today the schema is keyed by `account_id`, but the engine, commands and UI
assume one account (`primary_account()`).

## Decision (proposed)
- `AccountRegistry` in `AppState`: `account_id → AccountRuntime`
  (`TokenManager`, engine task, IMAP IDLE task, `StatusCell`, background job).
  All runtimes run all the time; the selected account is only a UI concept.
- Shared: SQLite connection, HTTP client, notifier. A global semaphore
  allows one backfill at a time; live delta syncs run in parallel.
- Commands and events carry `accountId`; `null` means "all" for unified views.
- UI: account rail with color rings and unread badges, Ctrl+1…9, Ctrl+0 for
  the unified inbox, per-account view state kept in memory.
- Details: [ARCHITECTURE.md](../ARCHITECTURE.md#multi-account-target-architecture),
  [ROADMAP A1](../ROADMAP.md#a1).

## Alternatives
- One process per account: simple isolation but N × RAM and N tray icons. Rejected.
- Sync only the selected account and poll the rest: misses the "notify
  even when not selected" requirement. Rejected.

## Consequences
One IMAP socket per account (Gmail allows 15). Every command signature
changes; do it behind the registry with one account first.
