# 0007: A panic never stops sync

- Status: accepted (2026-10-21)

## Context
A panic in the OTP extractor killed the engine task halfway through the
first sync: only 7 messages were stored, no history id was saved, and the UI
showed "Checking for mail…" forever. The release profile also had
`panic = "abort"`, which would have killed the whole tray app.

## Decision
- Each sync pass (`Engine::run_once`), the background job and each IMAP
  connection attempt run under `catch_unwind`. On a panic: log it, add an
  activity entry, reset `syncing` / `progress`, set a friendly error, and
  continue on the next wake.
- `otp::extract` is additionally wrapped (`mime::safe_otp`) so one bad
  message only loses its code, not the message.
- Release profile uses `panic = "unwind"`.

## Consequences
A bug degrades one pass or one message instead of the whole app. Code
inside these tasks must stay unwind-safe in practice (`AssertUnwindSafe`;
shared state is behind poison-tolerant locks).
