# 0015: Email rendering: Rust sanitizer, offline frame, hash-pinned script

- Status: accepted (2026-10-21)

## Context
HTML mail is hostile input. The old renderer stripped a few tags with
regexes in TypeScript and put the result in an `allow-same-origin` iframe so
the parent could size it and catch link clicks. Remote images were allowed
straight from the frame after "Show images", which leaks the IP address and
the open time to the sender, and `cid:` images never rendered.

## Decision
`render_message(messageId, dark, showImages)` (Rust, `src-tauri/src/render`)
returns a complete `srcdoc`:

1. **Sanitize with ammonia** (allowlist). Scripts, handlers, forms, frames,
   objects, `meta`, `base`, `link`, `svg`/`math` are dropped; `title`,
   `textarea`, `template` and similar have their content removed; links keep
   only http/https/mailto and get `rel="noopener noreferrer"`; relative URLs
   are denied. `<style>` blocks are extracted first and cleaned by
   `render::css` (no `url()` except `data:image`, no `@import`/`@font-face`,
   no `expression()`/`behavior`/`-moz-binding`/`image-set()`, declarations
   with CSS escapes dropped, `position: fixed|sticky` made static, no `<`).
2. **Images never load from the frame.** A pre-scan lists remote images and
   flags tracking pixels (tiny size, hidden, or known tracker URLs).
   `img src` is rewritten by an attribute filter to: a `data:` URL from the
   attachments table (`cid:`), a `data:` URL fetched by the Rust image proxy
   (only when the user clicked *Show images*, allowlisted the sender, or set
   *Images: Always*), or a 1x1 blank. Trackers are never fetched.
3. **Image proxy** (`render::proxy`): no cookies, no Referer, public hosts
   only (IP literals and DNS answers in private/loopback/link-local/CGNAT
   ranges are refused through a custom resolver), ports 80/443, at most 3
   redirects, bytes sniffed (PNG/JPEG/GIF/WebP/AVIF/BMP/ICO only, no SVG),
   5 MB each, 25 MB and 40 images per message, 48 MB in-memory cache.
4. **Frame document**: CSP meta `default-src 'none'; img-src data:;
   style-src 'unsafe-inline'; font-src data:; script-src 'sha256-…'` plus
   `form-action`/`base-uri`/`frame-src`/`connect-src 'none'`.
   `body dir="auto"` and `unicode-bidi: plaintext` on blocks so Arabic
   paragraphs lay out right to left inside English mail.
5. **Sandbox**: `sandbox="allow-scripts"` **without** `allow-same-origin`.
   The frame is an opaque origin: no access to the app DOM, storage, cookies
   or the Tauri IPC bridge, no popups, no top navigation, no forms.
6. **One frame script, pinned by hash** (`render::FRAME_SCRIPT`). It posts
   `{pm:1,t:"h"}` height updates (auto-height, no inner scrollbar) and
   `{pm:1,t:"link",href,text}` on clicks, and folds quoted text
   (`.gmail_quote`, `blockquote[type=cite]`, `.yahoo_quoted`, plain-text `>`
   quotes) behind a "•••" toggle. The parent only trusts messages whose
   `source` is that iframe's window. The same hash is in the app CSP in
   `tauri.conf.json` (a `srcdoc` frame inherits the parent policy); a unit
   test fails if the two drift.
7. **Links**: the app compares the domain shown in the link text with the
   real host and warns on a mismatch or punycode host before opening the
   system browser (`lib/links.ts`). `open_external` still only accepts
   http/https/mailto.
8. **Dark mode**: "smart" inverts the whole email (`invert(1)
   hue-rotate(180deg)`) and re-inverts images; "original" shows the sender's
   colors on white. Plain text gets real dark colors (no filter). Printing
   always uses original colors.
9. **Plain text** mail (no real HTML part; mail-parser's generated HTML is
   ignored) is escaped, linkified and gets `>` quotes as nested blockquotes.

## Why `allow-scripts` at all
The brief asked for a sandbox without scripts. Without `allow-same-origin`
the parent can't read the frame's height or intercept its clicks, and
without a script the frame can't tell it either, so the reader would need
an inner scrollbar and links would be dead. The compromise keeps the
dangerous half (`allow-same-origin`) off and lets exactly one known script
run: email scripts are removed by the sanitizer *and* blocked by both CSPs
(no `'unsafe-inline'`, inline handlers need `'unsafe-hashes'`).

## Consequences
- Rendering moves to Rust (testable: `render/tests.rs` has hostile samples:
  handlers, `javascript:` variants, mXSS-style nesting, `meta refresh`,
  `base`, forms, CSS escapes, trackers).
- `FRAME_SCRIPT` changes need the new hash in `tauri.conf.json`
  (`cargo test` says so).
- CSS `background-image` from the web is never shown, even with images on.
- Remote images are re-fetched after a restart (memory cache only).
