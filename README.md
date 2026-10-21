# Pulse Mail

A light, always-on Gmail client for Windows.

- **Lives in the tray from login**, with a small Rust core and no window until you want one.
- **New mail in seconds:** an IMAP IDLE push connection wakes a Gmail history sync, then a native Windows toast appears.
- **Verification codes, one click:** OTP emails get a **Copy code** button right on the toast.
- **Instant reading:** everything is cached locally (SQLite + full-text search), so opening a message never waits on the network.
- **Looks like Gmail, only cleaner:** light and dark themes, dense lists, Gmail keyboard shortcuts.
- **Private by design:** your own Google OAuth client, the refresh token in Windows Credential Manager, no servers, no telemetry.

> Status: **Phase 1 in progress.** The core, sync, toasts and UI shell are built and
> tested offline; first real sign-in is waiting on the OAuth client setup. See
> [docs/PLAN.md](docs/PLAN.md).

## Quick start

Requirements (Windows 10/11): Rust stable (MSVC) via rustup, Visual Studio
Build Tools with C++, WebView2 Runtime (preinstalled on Windows 11),
Node 22+, pnpm.

```powershell
pnpm install
pnpm dev                     # UI only, in the browser, with mock data
pnpm tauri dev               # the real app (needs the OAuth client below to sign in)
pnpm tauri build             # per-user NSIS installer in <target dir>/release/bundle/nsis
```

Before the first sign-in, create your Google OAuth client and save it to
`%APPDATA%\com.abdallah.pulsemail\google-oauth-client.json`:
**[docs/SETUP-GOOGLE-OAUTH.md](docs/SETUP-GOOGLE-OAUTH.md)** (about 10 minutes, one time).

## Tests

```powershell
pnpm test                                              # UI unit tests (vitest)
pnpm build                                             # typecheck + production bundle
cargo test --manifest-path src-tauri/Cargo.toml        # Rust unit tests (offline)
```

## Docs

| Doc | What's in it |
| --- | --- |
| [AGENTS.md](AGENTS.md) | Rules, commands and layout for anyone (human or AI) changing the code |
| [docs/PLAN.md](docs/PLAN.md) | Phases, acceptance criteria, performance targets, progress |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | How it works, with a diagram |
| [docs/DESIGN.md](docs/DESIGN.md) | Visual spec: layout, colors, type, shortcuts |
| [docs/SECURITY.md](docs/SECURITY.md) | Where secrets live and how email HTML is contained |
| [docs/SETUP-GOOGLE-OAUTH.md](docs/SETUP-GOOGLE-OAUTH.md) | Step-by-step Google Cloud setup |
| [docs/decisions/](docs/decisions/) | Architecture decision records |

## Where things are stored

| What | Where |
| --- | --- |
| OAuth client (yours) | `%APPDATA%\com.abdallah.pulsemail\google-oauth-client.json` |
| Refresh token | Windows Credential Manager (`com.abdallah.pulsemail`) |
| Mail cache | `%APPDATA%\com.abdallah.pulsemail\pulse-mail.db` |
| Logs | stderr in dev (`PULSE_MAIL_LOG=debug` for more) |

Personal project; not affiliated with Google. Gmail is a trademark of Google LLC.
