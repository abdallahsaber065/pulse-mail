# Cutting a release

For the person tagging `vX.Y.Z` on `main` (see also [AGENTS.md](../AGENTS.md)).

## Google OAuth secrets (required on this repo)

Published installers bake in the project's Google Cloud **Desktop** OAuth
client at compile time (`option_env!("PULSE_GOOGLE_CLIENT_ID")` /
`option_env!("PULSE_GOOGLE_CLIENT_SECRET")` in
`src-tauri/src/auth/client_config.rs`). Add both as **repository** Actions
secrets (not a GitHub Environment):

| Secret name | Value |
| --- | --- |
| `PULSE_GOOGLE_CLIENT_ID` | The desktop client ID, ending in `.apps.googleusercontent.com` |
| `PULSE_GOOGLE_CLIENT_SECRET` | The desktop client secret from the same JSON |

Settings → Secrets and variables → Actions:

https://github.com/abdallahsaber065/pulse-mail/settings/secrets/actions

The release workflow passes them into the Tauri build env and never prints
them. Forks without the secrets get a warning and still build (those
binaries need a local `google-oauth-client.json`). A release on this repo
**fails** if either secret is missing, so we don't ship an installer that
can't sign in.

Also in Google Cloud (Auth Platform → Audience): set the OAuth consent
screen to **In production**. While it is **Testing**, Google expires refresh
tokens after 7 days. Unverified + the Gmail restricted scope is still capped
at about 100 users and still shows a warning screen — that is expected until
you apply for Google verification.

## Tag

1. Version is identical in `package.json`, `src-tauri/tauri.conf.json`,
   `src-tauri/Cargo.toml` (refresh `Cargo.lock` with `cargo check`).
2. Update `CHANGELOG.md`.
3. Merge to `main`, then `git tag -a vX.Y.Z` and `git push origin vX.Y.Z`.
4. The Release workflow builds Windows / macOS (Apple Silicon) / Linux into
   a draft, then publishes it. Write the GitHub Release notes (`gh release
   edit`) — the body starts empty.

Updater signatures use `TAURI_SIGNING_PRIVATE_KEY` and
`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (already in Actions secrets).
