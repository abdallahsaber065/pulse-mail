# Architecture Decision Records

Short, numbered, never rewritten: later changes go under "Updates" in the
ADR or into a new ADR that supersedes it. Template: Context → Decision →
Consequences (+ Alternatives when useful), under about 40 lines.

| # | Decision | Status |
| --- | --- | --- |
| [0001](0001-tauri-webview2-ui.md) | Tauri v2 + WebView2 UI, not a pure-Rust GUI | accepted |
| [0002](0002-imap-idle-doorbell-rest-history.md) | IMAP IDLE doorbell + Gmail REST history | accepted (updated) |
| [0003](0003-rustls-ring.md) | rustls + ring for all TLS | accepted |
| [0004](0004-window-close-behavior.md) | Hide on close by default, destroy as option | proposed (needs measurement) |
| [0005](0005-winrt-toasts.md) | WinRT toasts from Rust | accepted |
| [0006](0006-metadata-first-batch-sync.md) | Metadata-first first sync through the Gmail batch endpoint | accepted |
| [0007](0007-panic-isolation.md) | A panic never stops sync | accepted |
| [0008](0008-char-boundary-safe-text.md) | All text slicing is char-boundary safe | accepted |
| [0009](0009-in-app-dialogs-and-snackbars.md) | In-app dialogs and snackbars, no native browser dialogs | accepted |
| [0010](0010-manifest-for-all-targets.md) | Common-Controls manifest embedded in every Windows target | accepted |
| [0011](0011-local-first-label-changes.md) | Local-first label changes with rollback and optimistic UI | accepted |
| [0012](0012-multi-account.md) | Multi-account: one runtime per account, all live | accepted |
| [0013](0013-reading-pane-modes.md) | Reading pane modes (right / bottom / none / window) | proposed |
| [0014](0014-full-history-and-storage.md) | Full-history backfill with a storage budget | accepted |
| [0015](0015-email-rendering-pipeline.md) | Email rendering: Rust sanitizer, offline frame, hash-pinned script | accepted |
| [0016](0016-quiet-background-checks.md) | Background checks are quiet; progress UI only for long or user-started work | accepted |
| [0017](0017-ipc-allowlist.md) | Per-command IPC allowlist, least-privilege capability | accepted |
| [0018](0018-onboarding.md) | Multi-step onboarding that chooses the download range up front | accepted |
