# 0020: Rich compose editor: TipTap 3 core

- Status: accepted (2026-10-21, `498cf18`)

## Context
Compose was plain text. Requirements: SolidJS, small bundle, clean paste,
RTL, lists, links, bold, italic, underline and quote, a toolbar plus
shortcuts, sanitized HTML with a plain-text alternative, autosaved drafts.

## Options
| Option | Notes |
| --- | --- |
| **TipTap 3** (`@tiptap/core` + `starter-kit`, ProseMirror) | Framework-agnostic core, so no Solid binding is needed. The schema drops unknown markup on paste. `textDirection: "auto"` gives `dir="auto"` on every block. StarterKit 3 includes underline and link. About 104 KB gzip |
| Lexical | Smaller core, but no official Solid binding (React-first). Lists, links and paste need several packages and more glue |
| Quill 2 | Simple, but has its own delta model, weaker RTL per block, and its HTML output is harder to keep clean |
| Raw `contenteditable` + `execCommand` | Tiny, but `execCommand` is deprecated, and paste and lists vary by engine. Too much to own |

## Decision
TipTap 3 vanilla core in `components/RichEditor.tsx`. Toolbar: bold,
italic, underline, strike, bullets, numbers, quote, link (Ctrl+K inline
form) and clear formatting. Standard shortcuts work. The editor emits HTML and text.
Rust re-sanitizes the HTML with an ammonia allowlist (`sanitize_compose_html`:
basic blocks, inline marks, lists, `a[href]` with http/https/mailto only,
and `dir`) and builds `multipart/alternative` with the text part.

Attachments: file picker or drag-and-drop. Bytes go through Tauri's raw IPC
body (`stage_attachment`, no base64) into a memory cache capped at 300 MB.
Gmail's 25 MB total and 50 files are enforced in the UI and in Rust. Drafts
over 3 MB use Gmail's `/upload` endpoint. Forward re-attaches the
original's non-inline files, from disk when stored and otherwise from Gmail.

## Consequences
The bundle grows by about 100 KB gzip, which is acceptable for a desktop app.
Inline images in the body are not supported yet.
