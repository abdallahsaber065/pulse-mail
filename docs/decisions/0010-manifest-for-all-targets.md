# 0010: Embed the Common-Controls manifest in every Windows target

- Status: accepted (2026-10-21)

## Context
`cargo test` started failing on Windows with `STATUS_ENTRYPOINT_NOT_FOUND`
(0xc0000139). Tauri embeds its app manifest (Common Controls v6, needed for
`TaskDialogIndirect`) only into the app binary; the test harness links the
same code without it.

## Decision
`build.rs` disables Tauri's manifest (`WindowsAttributes::new_without_app_manifest`)
and embeds `src-tauri/windows-app-manifest.xml` (Common Controls v6 + per-monitor
DPI awareness) into every linked target with `/MANIFEST:EMBED` and
`/MANIFESTINPUT`. The file is copied to `OUT_DIR` first because the repo
path contains spaces.

## Consequences
Tests, examples and the app all start. MSVC-only (the only target we build).
