# 0004: Hide on close by default; destroy-on-close as a setting

- Status: proposed (measure in phase 1)

## Context
Opening a hidden WebView2 window is instant; recreating it costs a few hundred
ms but frees about 60-100 MB.

## Decision (provisional)
Default to **hide** for the fastest reopen. Offer **destroy** as a setting
(`destroy_on_close`) that applies on the next close. Measure both against the
targets in `docs/PLAN.md` and make the final call. Phase 3 also tries
WebView2 `MemoryUsageTargetLevel::Low` while hidden.

## Consequences
The app must never rely on the window existing (true by design: Rust owns state).
