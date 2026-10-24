# ADR 0026: Always-on reliability — quiet hours, resume catch-up, local crash logs

- Date: 2026-10-24
- Status: accepted
- Scope: ROADMAP N2 (quiet hours), X2 (sleep/resume), V4 (crash logs)

## Context

Pulse Mail's whole value is being trustworthy 24/7: a code arrives, the toast
appears within seconds. Three gaps worked against that:

1. Notifications fire at any hour; nothing muted them at night.
2. After sleep/resume the IMAP IDLE socket is dead until its TCP timeout and
   backoff play out (minutes), so mail that arrived while the PC slept waits.
3. The installed app logs to nowhere — a misbehaving sync after days of
   uptime left no trace to diagnose.

## Decision

**Quiet hours (N2)** are a purely client-side window over the existing toast
decision point in `Engine::delta_sync`: prefs `quiet_start`/`quiet_end`
(minutes since local midnight, 30-minute steps, may cross midnight) and
`quiet_allow_otp`. Local time comes from `GetDynamicTimeZoneInformation`
(no new dependency; `windows-sys` is already in the tree). Mail keeps
syncing and counts keep updating — only toasts pause. Verification codes
stay allowed by default because they are time-critical.

**Resume catch-up (X2)** uses a clock-jump detector instead of Windows
power APIs: a task compares wall-clock elapsed against monotonic elapsed
every 30 s; a gap ≥ 90 s means the machine slept. This needs no window, no
new crate, and works in destroy-on-close mode where no window exists to
receive power events. On a jump it calls `sync_now(false)` — a quiet
background check — on every account. IMAP re-handshaking still rides the
existing reconnect backoff; the delta covers the missed mail immediately.
NTP-sized drift (< 90 s) never triggers it (unit-tested).

**Crash logs (V4)**: `tracing-appender` (small, same ecosystem as
`tracing-subscriber`) writes a rolling daily file to
`%LOCALAPPDATA%\com.abdallah.pulsemail\logs`, pruned to 7 days at startup —
size-capped with no extra state. A panic hook records every panic (sync
passes catch theirs; this sees all of them) and then chains to the default
hook. `open_logs_dir` (Settings → About) reveals the folder; it follows the
full IPC allowlist discipline (build.rs + capabilities + security tests).

Logs never contain message bodies or tokens — the existing logging rules
(security tests forbid formatting secrets; bodies were never logged) carry
over. Nothing leaves the PC.

## Consequences

- Quiet hours need one real-world check (set 10 PM–6:30 AM, send two mails:
  a code toasts, a regular mail doesn't) — added to VERIFY-LIVE.md.
- The resume detector polls a 30 s timer; it is the only new always-on task
  and costs nothing measurable.
- Non-Windows builds log to console only (`LOCALAPPDATA` unset); the
  project is Windows-only in practice.
- `tracing-appender`'s writer guard is intentionally leaked: the app runs
  until process exit, and a dropped guard would swallow the last log lines.
