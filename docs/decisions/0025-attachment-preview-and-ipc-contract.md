# 0025: On-demand attachment preview, Tauri v2 capability discipline, and reactive overlay scope

- Status: accepted (2026-10-23, `v0.2.0`)

## Context

Pulse Mail previously offered only direct "Open" (system default application) and
"Download" for attachments. For security, speed, and ergonomics, users needed instant
in-app preview for common documents and media (PDFs, images, code/text/csv/json,
and audio/video) without polluting the filesystem with untracked temp files.

Concurrently, during development of contact auto-completion and thread context menus,
two systemic bugs were uncovered and resolved:
1. Tauri v2 silently rejects IPC commands in the webview when omitted from capabilities,
   even if registered in `generate_handler!`.
2. SolidJS reactive prop evaluation in overlay menus fails if the component is torn down
   before the action consumes the prop.

## Decision

### 1. On-Demand Attachment Preview
- **Streaming over IPC**: Added `get_attachment_data` command in `src-tauri/src/commands/mail.rs`.
  It reads cached bytes from SQLite / disk files or pulls the raw message from Gmail on demand,
  returning standard base64 without creating temporary files.
- **Frontend Viewer (`components/AttachmentPreview.tsx`)**:
  - **Images**: Responsive zoom controls, scale-to-fit, and transparent checkerboard backing.
  - **PDF Documents**: Embedded Chromium/WebView2 PDF viewer via iframe (`#toolbar=1`),
    supporting pan, zoom, page thumbnails, and text selection.
  - **Code & Text**: UTF-8 decoded monospace viewer with line numbers, copy-all button,
    and word-wrap toggle (`.txt`, `.md`, `.json`, `.csv`, `.ts`, `.py`, `.rs`, `.sql`, etc.).
  - **Audio/Video**: Native HTML5 `<audio>` and `<video>` player controls.
  - **Fallback Card**: Clean metadata card with direct "Open in App" and "Download" buttons.
- **Memory Safety**: `URL.createObjectURL(blob)` is created on demand and strictly revoked via
  `URL.revokeObjectURL(url)` in `onCleanup` on modal unmount.

### 2. Tauri v2 Capability Discipline
- Tauri v2 requires explicit capabilities in `capabilities/default.json` and build-time
  registration in `src-tauri/build.rs`.
- Commands registered only in `generate_handler!` fail silently in the webview with a capability
  rejection.
- Enforced automated gate: `cargo test security::tests::command_allowlist_in_sync` guarantees
  100% parity across `build.rs`, `commands.rs`, and `capabilities/default.json`.

### 3. SolidJS Reactive Scope in Overlays
- In floating menus, popovers, and context menus, calling `onClose()` sets the parent
  state signal (e.g. `setContextMenu(null)`), immediately unmounting the component.
- Any subsequent access to reactive props (`p.thread`) evaluates against destroyed scopes,
  throwing a `TypeError` and silently aborting asynchronous operations.
- Required pattern:
  ```typescript
  const exec = (fn: (t: ThreadRow) => void) => {
    const thread = { ...p.thread };
    try {
      fn(thread);
    } finally {
      p.onClose();
    }
  };
  ```

### 4. CSS Stacking Context on Sibling Flex Rows
- Sibling container rows (`To`, `Cc`, `Subject`, `Body editor`) establish stacking contexts.
  In standard DOM painting order, subsequent siblings paint over earlier siblings.
- Floating typeahead menus anchored to `To` must set `z-40` on the active row, preventing
  the dropdown from rendering behind the editor.

## Consequences
- Rich, zero-temp-file preview for all standard attachment types.
- Strict compiler and capability test guarantees against silent IPC drops.
- Deterministic, crash-free execution of thread actions from context menus.
