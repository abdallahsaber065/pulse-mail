# Contributing to Pulse Mail

Thanks for wanting to help. Pulse Mail is an always-on Gmail client
(Tauri v2 + Rust + SolidJS). Windows is the daily-driver platform; macOS
(Apple Silicon) and Linux are experimental but CI runs on all three.

## Setup

- **Node.js** 22+ and **pnpm**
- **Rust** stable via `rustup`
- Compilers:
  - Windows: Visual Studio Build Tools (C++) and WebView2 (preinstalled on Windows 11)
  - macOS: Xcode Command Line Tools
  - Linux: `build-essential`, `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`, `libssl-dev`

```bash
pnpm install
```

## Running it

### UI only (no Google account)

```bash
pnpm dev
```

Open [http://localhost:1420](http://localhost:1420). The app uses a
fictional demo mailbox (`src/lib/mock.ts`). See [docs/DEMO.md](docs/DEMO.md)
for `?toast=1`, `?shot=1`, and the optional `PULSE_MAIL_DEMO=1` Tauri path.

### Full desktop app

1. Follow [docs/SETUP-GOOGLE-OAUTH.md](docs/SETUP-GOOGLE-OAUTH.md) and put
   `google-oauth-client.json` in the repo root or the app config dir.
2. `pnpm tauri dev`

## How the code is split

- **Rust owns state:** SQLite cache, sync (IMAP IDLE + Gmail REST), OAuth,
  tokens in the OS credential store.
- **SolidJS is a renderer:** it calls `#[tauri::command]`s through
  [`src/lib/api.ts`](src/lib/api.ts).
- Email HTML only renders in [`EmailFrame.tsx`](src/components/EmailFrame.tsx)
  (sandboxed iframe, remote images off until you click).

Rules that actually bite: [AGENTS.md](AGENTS.md). Architecture:
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Before you open a PR

```bash
pnpm test
pnpm build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
cargo fmt --manifest-path src-tauri/Cargo.toml --all --check
```

CI also runs `pnpm audit --audit-level high` and clippy with `-D warnings`.

- One fix or feature per PR, conventional commits (`feat:`, `fix:`, `docs:`).
- Tests next to the code (`*.test.ts` / `#[cfg(test)]`) for anything pure.
- Never commit OAuth JSON, `.env`, or `*.db`.

New `#[tauri::command]`s need three registrations (handler, `build.rs`
`COMMANDS`, `capabilities/default.json`). The test
`security::tests::command_allowlist_in_sync` fails if you miss one.

Cutting a GitHub Release (OAuth secrets, tags, notes): [docs/RELEASING.md](docs/RELEASING.md).
