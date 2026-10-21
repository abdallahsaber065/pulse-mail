# Security model

Pulse Mail holds full access to a Gmail account (`https://mail.google.com/`).
Treat everything below as requirements.

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
- Endpoints: `accounts.google.com`, `oauth2.googleapis.com`, `gmail.googleapis.com` (REST and `/batch/gmail/v1`), `imap.gmail.com`. Nothing else, no telemetry. Adding any endpoint requires asking the user.

**Rendering hostile email**
- Email HTML renders only in `EmailFrame`: `<iframe sandbox="allow-same-origin">`, so no scripts, forms, popups or top navigation.
- A CSP `<meta>` inside the frame: `default-src 'none'; img-src data: cid:; style-src 'unsafe-inline'; script-src 'none'; form-action 'none'; base-uri 'none'`.
- Remote images are blocked by default (tracking pixels). "Show images" only adds `https:` to `img-src` for that message.
- Scripts, event-handler attributes, `javascript:` URLs, iframes, objects, `<meta>` and `<base>` are stripped before rendering (defense in depth).
- Link clicks are intercepted and sent to `open_external`, which allows only `http`, `https` and `mailto` and opens the system browser. The referrer is not sent.

**App shell**
- App CSP in `tauri.conf.json`: `default-src 'self'`, no remote scripts, `object-src 'none'`, `freezePrototype: true`.
- Capability `core:default` only: the webview can't reach the filesystem, shell, clipboard or opener APIs directly. Everything goes through the reviewed commands.
- Single instance, per-user NSIS install, no admin rights.

## Known risks and decisions

- **Panics:** sync tasks catch panics (ADR 0007) so a malformed email can't
  stop notifications; the release build unwinds instead of aborting.

- **Unverified app with a restricted scope.** Fine for personal use (under 100 users) once the app is set to "In production". In "Testing" the refresh token dies after 7 days.
- **OTP codes on toasts** can show on the lock screen if Windows is set to allow it. "Hide codes on the lock screen" is planned ([ROADMAP O1](ROADMAP.md#o1)). Until then, turn off Settings → System → Notifications → "Show notifications on the lock screen" for Pulse Mail if that matters.
- **Clipboard:** copied codes stay on the clipboard (and in Windows clipboard history if it's enabled). Planned: clear after 60 s ([ROADMAP O1](ROADMAP.md#o1)).
- **Local cache is not encrypted at rest.** It relies on the Windows user account and BitLocker. Options are in [ROADMAP V2](ROADMAP.md#v2) (SQLCipher or DPAPI).

## Reporting / checking

Before each phase sign-off:
- `rg -n "expose\(\)" src-tauri/src` and confirm each use is a request body or header.
- `rg -n "println!|dbg!" src-tauri/src` should return nothing.
- `git status` shows no `*.json` client file and no `*.db`.
