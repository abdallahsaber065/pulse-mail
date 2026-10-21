# 0005: Toasts through tauri-winrt-notification, not the notification plugin

- Status: accepted (2026-10-21)

## Context
"Copy code" must work straight from the toast, even when no window or
webview exists. `tauri-plugin-notification` 2.6+ supports desktop actions, but
it's designed around the JS API and permission model.

## Decision
Call `tauri-winrt-notification` (the WinRT wrapper the plugin itself uses)
directly from Rust: buttons `copy` / `open`, `on_activated` handled in Rust
(clipboard write, open thread).

## Consequences
- Works with zero windows; nothing exposed to the webview.
- Windows-only code path (`#[cfg(windows)]`); fine for this app.
- Like the plugin, dev builds show PowerShell as the sender; installed builds
  use the app's AppUserModelID (the bundle identifier).
- Activation is delivered while the toast is on screen; clicks from Action
  Center after the process restarts need COM activation (phase 3 if needed).
