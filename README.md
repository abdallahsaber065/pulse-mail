# Pulse Mail

A light, always-on Gmail client for Windows.

- **Lives in the tray from login**, with a small Rust core and no window until you want one.
- **New mail in seconds:** an IMAP IDLE push connection wakes a Gmail history sync, then a native Windows toast appears.
- **Verification codes, one click:** OTP emails get a **Copy code** button right on the toast.
- **Fast from the first second:** the newest 50 inbox messages show 1–2.5 s after sign-in; older mail fills in in the background; everything is cached locally (SQLite + full-text search).
- **Transparent:** a status chip and progress bar show exactly what sync is doing ("Syncing 120/300", "Live · just now"), with an activity log.
- **Looks like Gmail, only cleaner:** light and dark themes, dense lists, Gmail keyboard shortcuts.
- **Private by design:** your own Google OAuth client, the refresh token in Windows Credential Manager, no servers, no telemetry.

> Status: **v0.1, Phase 1 nearly done**, running daily on the author's PC with
> one Gmail account. Reading and triage (archive, star, read/unread) work;
> compose, attachments, multiple accounts and full-history sync don't yet.
> Honest details: [docs/STATUS.md](docs/STATUS.md). What's next:
> [docs/ROADMAP.md](docs/ROADMAP.md).

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
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
cd src-tauri; cargo run --example sync_bench          # first-sync benchmark (uses your account, read-only)
```

How changes are made (gates, commits, dev console, logs): [docs/WORKFLOW.md](docs/WORKFLOW.md).

## Docs

| Doc | What's in it |
| --- | --- |
| [AGENTS.md](AGENTS.md) | Rules, commands and layout for anyone (human or AI) changing the code |
| [docs/STATUS.md](docs/STATUS.md) | What works today, what doesn't, known bugs, measured numbers |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Prioritized feature catalog (P0/P1/P2/later) with design notes |
| [docs/PLAN.md](docs/PLAN.md) | Phases, acceptance criteria, performance targets, progress log |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Modules, data flow, sync state machine, IPC, schema, multi-account target |
| [docs/DESIGN.md](docs/DESIGN.md) | Visual spec: layout, components, colors, motion, shortcuts, planned layouts |
| [docs/WORKFLOW.md](docs/WORKFLOW.md) | How we work: docs first, gates, benchmarks, dev run, agents |
| [docs/SECURITY.md](docs/SECURITY.md) | Where secrets live and how email HTML is contained |
| [docs/SETUP-GOOGLE-OAUTH.md](docs/SETUP-GOOGLE-OAUTH.md) | Step-by-step Google Cloud setup |
| [docs/decisions/](docs/decisions/) | Architecture decision records |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Contribution guide, setup, and PR conventions |

## Where things are stored

| What | Where |
| --- | --- |
| OAuth client (yours) | `%APPDATA%\com.abdallah.pulsemail\google-oauth-client.json` |
| Refresh token | Windows Credential Manager (`com.abdallah.pulsemail`) |
| Mail cache | `%APPDATA%\com.abdallah.pulsemail\pulse-mail.db` |
| UI preferences (theme, density) | WebView2 `localStorage` |
| Logs | stderr in dev, teed to `%TEMP%\pulse-mail-dev.log` by the dev console (`PULSE_MAIL_LOG=debug` for more); no log file in release yet |

## Contributing

Contributions are welcome! Please check out [CONTRIBUTING.md](CONTRIBUTING.md) for local setup, development guidelines, and PR checklists.

## License

[MIT](LICENSE) © 2026 Abdallah Saber.

Personal project; not affiliated with Google. Gmail is a trademark of Google LLC.
