# 0013: Reading pane modes as a layout setting

- Status: proposed (2026-10-21)

## Context
The user wants Gmail-like options: open mail in a side pane, or full page in
place of the list. Today the split is fixed (right pane at ≥ 1024 px, reader
replaces the list below that).

## Decision (proposed)
- Setting `layout = right | bottom | none` (Gmail calls `none` "No split"),
  default `right`; quick toggle in the top bar.
- `ThreadList` and `Reader` stay unchanged; only the container in `App.tsx`
  changes. In `none`, opening a thread swaps the list for the reader, keeps
  the list's scroll position and cursor, and `j`/`k` move to the
  next/previous thread while reading.
- "Open in new window" (Shift+Enter) creates a second webview window with
  label `thread-<id>`; it reads from the same core.
- Pane size (right/bottom) is resizable and remembered.
- Details: [DESIGN.md](../DESIGN.md#reading-pane-modes), [ROADMAP R1](../ROADMAP.md#r1).

## Consequences
Extra windows cost about 30–60 MB each while open; they close independently
and never hold state.
