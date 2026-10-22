# Contributing to Pulse Mail

Thank you for your interest in contributing to Pulse Mail!

Pulse Mail is an always-on, privacy-first Gmail client for Windows built with **Tauri v2**, **Rust**, and **SolidJS + TypeScript**.

---

## 🛠️ Development Setup

### Prerequisites
- **OS**: Windows 10 or 11 (64-bit)
- **Node.js**: v22+ with **pnpm** (`npm i -g pnpm`)
- **Rust**: Stable via `rustup` with the MSVC toolchain
- **Visual Studio Build Tools**: C++ build tools installed
- **WebView2 Runtime**: Installed natively on Windows 11 / modern Windows 10

---

## 🚀 Running the App Locally

### 1. Frontend-Only Mode (Recommended for UI & Design contributions)
You do **not** need a Google Cloud account or Rust compilation to work on the UI, components, or styles:
```powershell
pnpm install
pnpm dev
```
Open [http://localhost:1420](http://localhost:1420) in any web browser. The app runs against a built-in mock dataset (`src/lib/mock.ts`) with hot module reloading.

### 2. Full Desktop App (Tauri dev)
To run the native Windows application with real Gmail integration:
1. Follow the [Google OAuth Setup Guide](docs/SETUP-GOOGLE-OAUTH.md) to obtain your desktop client ID & secret.
2. Place the file at `%APPDATA%\com.abdallah.pulsemail\google-oauth-client.json`.
3. Launch the desktop app:
```powershell
pnpm tauri dev
```

---

## 📐 Architecture & Principles

- **Rust owns all state**: SQLite cache (`rusqlite`, FTS5), sync engine (IMAP IDLE + Gmail REST API), OAuth loopback, and token storage in Windows Credential Manager.
- **Stateless UI**: SolidJS + TypeScript calling `#[tauri::command]`s in `src-tauri/src/commands/` via `src/lib/api.ts`.
- **Security First**:
  - Tokens never leave memory or Windows Credential Manager.
  - Email HTML is rendered strictly inside a sandboxed iframe ([`EmailFrame.tsx`](src/components/EmailFrame.tsx)) with remote images blocked by default.
  - No analytics, telemetry, or external third-party network calls.

---

## 🧪 Verification & Quality Gates

Before submitting a PR, make sure all tests pass and formatting is clean:

```powershell
# Frontend tests & typecheck
pnpm test
pnpm build

# Rust tests & linter
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets
```

---

## 📋 Pull Request Guidelines

1. **Keep it focused**: One bug fix or feature per pull request.
2. **Commit Messages**: Use [Conventional Commits](https://www.conventionalcommits.org/) format (e.g. `feat(search): add syntax autocomplete`, `fix(sync): smooth raw download rate limits`).
3. **Tests**: If adding business logic or UI state helpers, add a unit test alongside the code (`*.test.ts` or `#[cfg(test)]` in Rust).
4. **Never Commit Credentials**: Never include `.json` OAuth keys, `.env` files, or `.db` databases.

Thank you for making Pulse Mail better!

