# ADR 0028: One token manager per account; the credential store is the source of truth

- Date: 2026-11-01
- Status: accepted
- Scope: `auth/`, `commands::sign_in` / `sign_out` / `resume`, `sync`
- Trigger: sign-ins dying after a few days and needing a restart or a full
  sign-in even after a reconnect had been made.

## Context

The refresh token lived in Windows Credential Manager, but each sync
controller built its own `TokenManager` from whatever the store held when it
started. That allowed three problems:

- A reconnect (a second `sign_in`) created a second in-memory copy while the
  old engine could still be refreshing; whichever finished last wrote the
  store, so an older credential could overwrite a newer one.
- A rotated refresh token (Google may return a new one on any refresh) was
  discarded, so the process kept a credential the server no longer accepted.
- Sign-out deleted the store entry while a refresh could be in flight; the
  refresh then wrote the deleted credential back.

None of this is visible in the UI until sync stops with "Your Google sign-in
expired", which is hard to attribute.

## Decision

- **Source of truth:** the credential store entry `refresh-token:<email>`
  holds the refresh token. Access tokens are memory-only. Nothing else is
  durable.
- **One owner:** `AppState.tokens` maps the mailbox email (exactly the
  credential store key) to one `Arc<TokenManager>`, created on first use and
  shared by that account's sync engine, IMAP IDLE task and every command.
  Accounts never share a manager, and a manager's `email` is immutable.
- **Serialized writes:** a manager's mutex is held across the token request and
  across every write to its store entry, so one account never runs two
  refreshes and never interleaves a renewal with a reconnect. Other accounts
  are independent.
- **Rotation:** a refresh-token value returned by Google is written back to the
  store (best effort, logged on failure) and used from then on.
- **Reconnect:** `sign_in` hands the new credential to the same manager
  (`adopt`), which saves it and swaps it in under that lock, and drops the
  previous grant's access token. `invalid_grant` re-reads the store once, so a
  credential renewed by another instance is picked up without a restart.
- **Sign-out:** `close()` is an atomic flag set before the store entry is
  deleted; a refresh already in flight checks it after the response and neither
  serves nor rewrites the credential. The next sign-in gets a fresh manager
  (`is_closed` is checked when looking one up).

## Consequences

- In-process there is a single credential writer per account, keyed by the
  same email as the store entry; account separation is the same key in memory
  and on disk.
- Cross-process safety relies on the single-instance plugin (a second launch
  focuses the running window, it does not start a second process). Two
  processes writing the same entry is not supported.
- A refresh already in flight delays `adopt` by at most the HTTP timeout; it
  only ever delays the account being reconnected, never another account.
- If the write-back fails, the newer token still works until the app exits;
  the next launch falls back to the store value, which is the previous one.
