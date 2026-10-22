# 0023: Google account name and photo

- Status: accepted (2026-10-22; profile scopes and endpoints approved by the user)

## Decision

Request `openid profile` alongside the existing Gmail scope. Read the name
and picture from Google's fixed HTTPS UserInfo endpoint with the same access
token used for Gmail. Do not decode unverified ID tokens. Existing accounts
keep syncing with their current grant; reconnecting grants profile access
without deleting mail or signing out first.

Fetch profile information once after sign-in, in a bounded background task.
Mail sync and the sign-in result do not wait for a photo. No polling or new
dependencies. A failed optional profile lookup never breaks sign-in or sync.

Keep the Google name separately from the name entered in Settings; the user's
choice always wins. Before profile access is granted, cached own-address SENT
mail can supply the default name, with the existing send-as lookup as a last
fallback. Prefer frequency, then newest occurrence, then name for ties.

Rust downloads the photo without credentials, cookies or referrer. Only HTTPS
Googleusercontent hosts on port 443 are accepted; redirects are refused.
Profile JSON is capped at 16 KiB, photos at 256 KiB, requests at 8 seconds.
Only sniffed raster images become local data URLs, stored in SQLite and sent
to the UI. The webview never loads a remote avatar. Initials remain the
fallback if a photo is missing, blocked, or cannot be decoded.

Reference: [Google OpenID Connect](https://developers.google.com/identity/openid-connect/reference).
