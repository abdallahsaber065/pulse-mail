# AGENTS.md

Instructions for any coding agent or human working in this repo. Tool-neutral
on purpose: no rules here depend on a specific agent, IDE or harness.

## What this is

Pulse Mail: a personal, always-on Gmail client for Windows. It lives in the
tray 24/7, shows a toast within seconds of new mail, pulls one-time codes out
of verification emails ("Copy code" right on the toast), and opens any message
instantly from a local cache. Single user, single PC, never published.

Read before larger changes: `docs/PLAN.md` (phases + acceptance criteria),
`docs/ARCHITECTURE.md`, `docs/SECURITY.md`, `docs/DESIGN.md`, `docs/decisions/`.

## Stack

- Tauri v2 (Rust core) + SolidJS + TypeScript + Tailwind v4 (Vite SPA, no SSR)
- Rust owns all state: OAuth, tokens (Windows Credential Manager via `keyring`),
  SQLite cache (`rusqlite`, bundled, FTS5), sync (IMAP IDLE + Gmail REST history)
- The UI is a stateless renderer that calls `#[tauri::command]`s in
  `src-tauri/src/commands.rs` through `src/lib/api.ts`
- Package manager: **pnpm** only. Rust: stable via rustup (MSVC toolchain)

## Layout

```
src/                     SolidJS UI
  components/            TopBar, Sidebar, ThreadList (virtualized), Reader, EmailFrame...
  lib/                   api.ts (IPC + browser mock), types.ts, format.ts, email-frame.ts, shortcuts.ts, theme.ts
  styles/app.css         Tailwind + design tokens (light/dark)
src-tauri/src/
  lib.rs                 app setup, plugins, state, run loop
  commands.rs            IPC surface (friendly errors only)
  auth/                  client config file, PKCE loopback OAuth, keyring, Secret type
  store/                 SQLite schema, migrations, queries
  sync/                  engine (mod.rs), gmail.rs REST, imap_idle.rs, mime.rs, status.rs
  notify/                Windows toasts (OTP "Copy code")
  otp/                   OTP extractor (pure, heavily tested)
  tray.rs, window.rs     tray menu, hide/destroy-on-close lifecycle
docs/                    plan, architecture, design, security, OAuth setup, ADRs
```

## Commands

Run from the repo root unless noted.

| Task | Command |
| --- | --- |
| Install JS deps | `pnpm install` |
| UI only, in a browser with mock data | `pnpm dev` (http://localhost:1420) |
| Full app (dev) | `pnpm tauri dev` |
| UI typecheck + build | `pnpm build` |
| UI unit tests | `pnpm test` |
| Rust check | `cargo check --manifest-path src-tauri/Cargo.toml` |
| Rust tests | `cargo test --manifest-path src-tauri/Cargo.toml` |
| One Rust module | `cargo test --manifest-path src-tauri/Cargo.toml otp::` |
| Lint | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets` |
| Format | `cargo fmt --manifest-path src-tauri/Cargo.toml` |
| Installer | `pnpm tauri build` (NSIS, per-user) |

Run the targeted tests for what you touched; run all of them before calling a
phase done. Never mark work done on a failing build or test.

## Hard rules

1. **No secrets in the repo, logs, or UI.** Refresh tokens live only in
   Windows Credential Manager. Access tokens live only in memory. The OAuth
   client file lives outside the repo (see `docs/SETUP-GOOGLE-OAUTH.md`).
   Wrap tokens in `auth::secret::Secret`; never `{:?}`-print request bodies,
   tokens, or raw mail. `.gitignore` covers `*.json` client files, `.env*`, `*.db`.
2. **Don't run anything that needs Google credentials** unless the user asks.
   Tests must be offline and deterministic (parse fixtures, in-memory SQLite).
3. **Rust owns state.** The UI never talks to the network or stores mail; it
   asks Rust over IPC. Don't expose plugin APIs to the webview
   (`capabilities/default.json` stays `core:default`).
4. **Email HTML is hostile.** Render only in `EmailFrame` (sandboxed iframe,
   no scripts, strict CSP, remote images off until the user clicks).
   Links open in the system browser through `open_external` (http/https/mailto only).
5. **User-facing text is friendly and non-technical.** Map errors to short
   sentences in `commands.rs` / `sync::SyncError::friendly`; details go to logs.
6. **Stay light.** Every new dependency must justify its RAM/binary cost
   (see perf targets in `docs/PLAN.md`). No new background polling loops.
7. **Schema changes** go in a new entry in `store/migrations.rs`; never edit a
   shipped migration.
8. **Ask before** adding scopes, network endpoints, telemetry, or anything
   that sends data anywhere except Google's Gmail/OAuth endpoints.

## Conventions

- Rust 2024 edition, `thiserror` for errors, `tracing` for logs
  (`PULSE_MAIL_LOG=debug` to raise verbosity). Keep `std::sync::Mutex`
  guards out of `.await` points.
- TypeScript strict; Solid signals/resources, no global store library.
- Tailwind utilities with the tokens in `src/styles/app.css`; follow
  `docs/DESIGN.md` for spacing, color and type. No new colors without a token.
- Pure logic gets unit tests next to it (`#[cfg(test)]` / `*.test.ts`).
- Record notable decisions as a short ADR in `docs/decisions/NNNN-title.md`.
- Update `docs/PLAN.md` progress when you finish a milestone.

## Git

Local repo only (no remote). Small, focused commits with imperative subjects.
Don't commit, rewrite history, or delete branches unless asked.
