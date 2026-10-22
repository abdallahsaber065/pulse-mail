# Security model

Pulse Mail holds full access to a Gmail account (`https://mail.google.com/`).
Treat everything below as requirements.

The user approved `openid profile` on 2026-10-22 for the Google account name
and photo (ADR 0023). Existing Gmail-only grants keep working until the user
reconnects; no silent consent upgrade or sign-out.

## Assets and where they live

| Asset | Where | Never in |
| --- | --- | --- |
| Refresh token | Windows Credential Manager, service `com.abdallah.pulsemail`, user `refresh-token:<email>` (via `keyring`) | files, SQLite, logs, UI, git |
| Access token (1 h) | process memory only (`TokenManager`, `Secret`) | anywhere persistent |
| OAuth client id/secret | `%APPDATA%\com.abdallah.pulsemail\google-oauth-client.json` (or `PULSE_MAIL_OAUTH_CLIENT`) | the repo (`.gitignore` blocks `google-oauth-client*.json`, `client_secret*.json`) |
| Cached mail | `%APPDATA%\com.abdallah.pulsemail\pulse-mail.db` (user profile, NTFS ACL) | git (`*.db` ignored) |

For Desktop-app clients Google treats the client secret as non-confidential
(PKCE protects the flow), but we still keep it out of the repo and logs.

## Controls

**Sign-in**
- OAuth 2.0 authorization code + PKCE S256, random 256-bit verifier.
- Loopback redirect on `127.0.0.1` with a random port, as Google recommends for desktop apps.
- Random `state` checked on return; other requests (favicon) are ignored; the listener closes after one code or 5 minutes.
- `prompt=consent&access_type=offline` so a refresh token is always issued.
- Sign-out (after an in-app confirm dialog) revokes the token at Google (best effort), deletes it from Credential Manager, and deletes the cached account data.

**Secrets in code**
- `auth::secret::Secret` formats as `***` in `Debug` and `Display`; the only accessor is `expose()` (grep for it when auditing).
- Errors returned to the UI are fixed friendly sentences; serde and HTTP error bodies are never echoed.
- Logs (`tracing`) carry no tokens, no message bodies, no subjects. Gmail errors log only the HTTP status and Google's machine-readable reason. Default level: `info`.

**Network**
- HTTPS only (`reqwest` with `https_only(true)`), rustls with the Windows certificate store through `rustls-platform-verifier`.
- IMAP over implicit TLS (993) with webpki roots.
- Endpoints: `accounts.google.com`, `oauth2.googleapis.com`, `gmail.googleapis.com` (REST and `/batch/gmail/v1`), `imap.gmail.com`; approved profile additions are `openidconnect.googleapis.com/v1/userinfo` and HTTPS subdomains of `googleusercontent.com` for the supplied photo. No telemetry. Adding any other endpoint requires asking the user.
- Profile data is requested only after sign-in/reconnect. Rust caps JSON at
  16 KiB and photos at 256 KiB, refuses redirects, sniffs raster formats and
  stores a data URL in SQLite. Photo downloads carry no OAuth token, cookies
  or referrer; the UI accepts only bounded raster data URLs. Failure leaves
  cached details/initials usable and never stops sync. Custom names remain
  separate from the Google default. No unverified ID token is used.

**Rendering hostile email** ([ADR 0015](decisions/0015-email-rendering-pipeline.md))
- Sanitized in Rust before the UI sees it: ammonia allowlist (no scripts,
  event handlers, forms, frames, objects, `<meta>`, `<base>`, `<link>`),
  links limited to http/https/mailto, CSS sanitized
  (remote `url()` rewritten to local data or dropped, no `expression()`,
  `behavior`, `-moz-binding`, no `position: fixed/sticky` overlays).
- Rendered as `srcdoc` in `<iframe sandbox="allow-scripts">` *without*
  `allow-same-origin`: the mail runs in an opaque origin, so it can't reach
  the app, its storage or the IPC bridge, open popups, submit forms or
  navigate the app.
- Frame CSP: `default-src 'none'`, images/fonts only `data:`, and the only
  script is our helper, pinned by SHA-256 in both the frame CSP and the app
  CSP (a test fails if they drift). Checked in Chrome: injected `<script>`,
  `onerror=` and `javascript:` payloads don't run.
- The frame never touches the network. Remote images are blocked by default;
  when allowed (once or per sender) Rust fetches them through a proxy that
  resolves only public IPs (no LAN/loopback, so no SSRF), sends no cookies
  or referrer, sniffs the type, caps size, and inlines them as `data:`.
  Known tracking pixels are dropped.
- Links go to `open_external` (http/https/mailto only) after a check that
  warns when the visible text names a different domain (phishing).
- Attachments: saved with Mark-of-the-Web (Zone.Identifier), executable and
  script types are never opened directly, file names are sanitized.

**App shell and IPC** ([ADR 0017](decisions/0017-ipc-allowlist.md))
- App CSP in `tauri.conf.json`: `default-src 'self'`, no remote scripts,
  `object-src 'none'`, `frame-src 'none'` (srcdoc frames are unaffected), `freezePrototype: true`.
- Per-command allowlist: `build.rs` declares every command
  (`AppManifest::commands`) and the capability grants only
  `core:event:allow-listen/unlisten` plus one `allow-<command>` each. No
  filesystem, shell, opener or clipboard plugin APIs reach the webview.
- Every command validates its arguments (ids, label ids, lengths, enum
  values; `set_pref` accepts only known keys and values).
- Devtools are compiled out of release builds (test `no_devtools_in_release`).
- Test `logging_never_formats_secrets` scans every `tracing` call for
  tokens, bodies, raw mail, HTML and codes.
- Access-token refresh is serialized per account (`TokenManager` mutex), so
  concurrent requests never race two refreshes.
- Single instance, per-user NSIS install, no admin rights.

**Sending mail** (`feee336`)
- The webview never builds MIME: it sends structured fields; Rust validates
  addresses (≤ 100 recipients, subject ≤ 500, body ≤ 1 MB) and builds the
  message with `mail-builder`, so header injection via CR/LF is impossible.
- The HTML alternative is generated from HTML-escaped plain text; quoted
  originals are escaped text, never the sender's HTML.
- Every send goes through a Gmail draft and waits the undo delay; nothing is
  sent by background code.
- Server search passes the user's query only to Gmail's `q` parameter;
  local FTS input is parsed into quoted terms (no raw FTS syntax).

**Verification codes**
- Not auto-copied unless the user turns it on. A copied code is cleared from
  the clipboard after 60 s (setting) if it's still there, and is excluded
  from Windows clipboard history and cloud clipboard.
- While the session is locked, toasts say a code arrived without showing it.

## Known risks and decisions

- **Panics:** sync tasks catch panics (ADR 0007) so a malformed email can't
  stop notifications; the release build unwinds instead of aborting.
- **Unverified app with a restricted scope.** Fine for personal use (under
  100 users) once the app is "In production". In "Testing" the refresh token
  dies after 7 days.
- **Local cache is not encrypted at rest.** It relies on the Windows user
  account and BitLocker. Next step: SQLCipher with a random key protected by
  DPAPI (CurrentUser) in Credential Manager ([ROADMAP V2](ROADMAP.md#v2)).
- **Several accounts** each have their own refresh token entry; removing an
  account deletes only that account's token and cached mail.

## Audit log

| Date | Check | Result |
| --- | --- | --- |
| 2026-10-21 | `cargo audit` (cargo-audit 0.22.2, 614 crates) | No vulnerabilities. 5 allowed warnings: `proc-macro-error` (RUSTSEC-2024-0370), `zbus_names`, `zvariant`, `zvariant_derive` (RUSTSEC-2026-0370/0371/0372) unmaintained, `glib` 0.18 unsound iterator (RUSTSEC-2024-0429). All are Tauri's Linux (GTK/D-Bus) dependencies, not compiled for Windows |
| 2026-10-21 | `pnpm audit` | No known vulnerabilities |
| 2026-10-21 | Secret scan of the repo (`client_secret`, `refresh_token` values, `ya29.`, `1//`) | Nothing committed |
| 2026-10-21 (late) | `cargo test` 107, `clippy -D warnings`, vitest 34, IPC allowlist test with the 6 new commands | Pass. No new dependencies since the audit above except `windows-sys` features (`DataExchange`, `Memory`, `Ole`). `cargo audit` and `pnpm audit` re-run: same result (no vulnerabilities, same 5 Linux-only warnings) |
| 2026-10-21 | Rendering: injected script, inline handler, `javascript:` link, meta refresh, form, remote CSS/image | All blocked (unit tests in `render/tests.rs` + manual check in Chrome) |

## Reporting / checking

Before each phase sign-off:
- `rg -n "expose\(\)" src-tauri/src` and confirm each use is a request body or header.
- `rg -n "println!|dbg!" src-tauri/src` should return nothing.
- `git status` shows no `*.json` client file and no `*.db`.
- `cargo test security::` passes (IPC allowlist, devtools, log scan).
- `cargo audit` and `pnpm audit` recorded in the audit log above.

## Compose HTML, attachment staging and stored files (2026-10-21)
- Compose HTML from the editor is re-sanitized in Rust (`sanitize_compose_html`,
  an ammonia allowlist: basic blocks and marks, lists, `a[href]` limited to
  http/https/mailto, `dir`). Scripts, styles, images and event handlers are dropped.
- Attachment bytes reach Rust as a raw IPC body (`stage_attachment`). The
  file name is percent-encoded in a header. Files are capped at 25 MB, the
  staging cache at 300 MB in memory, and nothing is written to disk.
  Staged entries are dropped after send.
- Saved attachments live under `%APPDATA%\…\files\<account>\<message>\<part>`.
  Path parts are restricted to alphanumerics, so traversal is impossible. They are
  deleted with the account on sign-out and count against the storage budget.
