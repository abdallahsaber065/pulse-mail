# 0009: In-app dialogs and snackbars, never native browser dialogs

- Status: accepted (2026-10-21)

## Context
Sign-out used `window.confirm`, which WebView2 shows as a grey
"localhost:1420 says…" box: ugly, unthemed, and it blocks the UI thread.

## Decision
- `components/ui/dialog.tsx`: a Material 3 dialog with scrim, enter/exit
  animation, focus trap, Esc/scrim to dismiss, safe button focused first.
  Promise API: `await confirmDialog({...})` / `showDialog({...})`.
- `components/Snackbar.tsx`: a queue (`snack(text, {action})`), one at a
  time, pauses on hover, optional action such as Undo.
- `alert`, `confirm` and `prompt` are not used anywhere (check with
  `rg "\b(alert|confirm|prompt)\(" src`).

## Consequences
Consistent look in light/dark, no extra dependency. Text input dialogs
(future `prompt` needs) get a variant of the same component.
