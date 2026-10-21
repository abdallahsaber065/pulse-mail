# 0011: Label changes are local first, with rollback and an optimistic UI

- Status: accepted (2026-10-21)

## Context
Archive, star and read/unread must feel instant, but they need a Gmail call.

## Decision
- Core (`Engine::modify_thread`): change `message_labels` and flags in
  SQLite, emit `mail-updated`, call `threads.modify`; on failure apply the
  inverse change and emit again. The error reaches the UI as a sentence.
- UI (`App.tsx`): an `overrides` map applied over fetched rows so the row
  changes in the same frame; cleared after success and the next reload,
  reverted on failure. Archive shows a snackbar with Undo (re-adds INBOX).
- Only known system labels and `Label_*` ids are accepted.

## Consequences
No offline queue yet: offline actions roll back
([ROADMAP X1](../ROADMAP.md#x1)). Label unread counts update on the next sync.
