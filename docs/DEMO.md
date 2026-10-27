# Demo mailbox

Pulse Mail's UI can run against a **fictional mailbox** so you can work on
layout, take screenshots, or record a GIF without a Google account. It never
touches OAuth, the keychain, SQLite, or Gmail.

## Browser (usual path)

```bash
pnpm install
pnpm dev
```

Open [http://localhost:1420](http://localhost:1420). The page talks to
`src/lib/mock.ts` instead of Tauri IPC (`isDemo` in `src/lib/api.ts`).

Useful query flags:

| URL | What you get |
| --- | --- |
| `/` | Inbox of made-up mail (`alex@example.com`) |
| `/?toast=1` | Same inbox, then a stand-in OTP notification (Copy code / Open) |
| `/?shot=1` | Hides the Demo chip, for screenshots and the README GIF |
| `/?thread=t0` | Opens the Northwind verification-code thread |
| `/?onboarding` | First-run welcome, no account |

Senders use `.example` / invented company names on purpose. Nothing in the
demo dataset is a real person or a real domain.

## Native window, still fake data

During a **dev** Tauri run only:

```bash
PULSE_MAIL_DEMO=1 pnpm tauri dev
```

Vite maps that to `VITE_PULSE_MAIL_DEMO`. `import.meta.env.DEV` is also
required, so a release installer cannot enable demo mode even if someone
sets the variable at runtime.

## What it does not do

- No Google sign-in, no IMAP, no SQLite writes
- Native tray / WinRT toasts are not driven (the `?toast=1` card is a UI
  stand-in so the OTP copy path can be shown)
- Compose/send/search-server are stubbed so the chrome still responds
