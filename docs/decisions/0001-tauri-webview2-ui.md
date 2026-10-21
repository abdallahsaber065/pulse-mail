# 0001: Tauri v2 + WebView2 UI, not a pure-Rust GUI

- Status: accepted (2026-10-21)

## Context
The goals are a 24/7 background client with the smallest idle footprint,
a UI that looks like Gmail, and correct rendering of real-world HTML
email. Options: egui/iced/Slint (pure Rust GPU UI), Dioxus Blitz, Electron, Tauri.

## Decision
Tauri v2 with a SolidJS + Tailwind SPA. All state lives in the Rust core.
The window is optional: the process runs with zero windows, and the window
can be hidden or destroyed on close (ADR 0004).

## Consequences
- HTML email renders correctly through WebView2 (shared, OS-updated Chromium),
  isolated in a sandboxed iframe.
- Idle footprint is the Rust process only when the window is destroyed.
- Pure-Rust GUIs would need a hand-written HTML/CSS email renderer, which is
  the hardest part of a mail client. Rejected.
- Electron ships its own Chromium and Node (100+ MB idle). Rejected.
