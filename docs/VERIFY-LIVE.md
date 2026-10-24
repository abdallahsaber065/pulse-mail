# Live verification checklist

Everything below is unit-tested and mock-verified but has **never run against
the real Gmail account** (see STATUS.md "untested live" entries). Work through
it top to bottom in one or two sessions on the real PC; tick the boxes and
note anything that misbehaves (then check the log with `PULSE_MAIL_LOG=debug`).

Run these against the **installed** build where noted (`pnpm tauri build`
once, or the draft release artifact) — dev builds show "PowerShell" as the
toast sender, so toast appearance/actions should be verified installed.

## 1. Send for real (compose path)

- [ ] Compose a plain text mail to yourself; it arrives in Gmail web.
- [ ] Rich text: bold/list/link survive; the plain-text alternative is sane
      (view original in Gmail web).
- [ ] Attach a file (picker and drag-drop); download it from Gmail web.
      Total under 25 MB; over 3 MB uses the upload endpoint.
- [ ] Reply and Reply all: the message lands in the same thread in Gmail web.
- [ ] Forward: original attachments re-attached.
- [ ] Undo send: cancel inside the grace window; the draft reopens and
      nothing was sent.
- [ ] Send from the second account via the From picker.
- [ ] Drafts: autosave creates a Gmail draft; close with Esc; reopen from
      Drafts and continue editing (existing-draft reopening is the newest path).
- [ ] `send-result` toast/snackbar shows success and failure states.

## 2. A real second account

- [ ] Add a second Google account via Account menu → Add another account.
- [ ] Onboarding runs: range estimate, notification choice, progress, done.
- [ ] Both accounts go "Live"; the chip shows per-account activity.
- [ ] "All inboxes" merges both by date; actions route to the right account.
- [ ] Mail to the **non-selected** account still toasts and names the account.
- [ ] Per-account notifications (all / codes only / off) behave independently.
- [ ] Remove the account: revoke, token and cache gone, first account intact.

## 3. Toasts and codes on live new mail (also M1)

- [ ] Send yourself a mail with a timestamp in the subject (e.g. from another
      provider or Gmail web); note send time vs `toast shown` in the log.
- [ ] Toast appears within the target (p50 ≤ 3 s, p95 ≤ 6 s over ~10 mails).
- [ ] OTP mail: "Copy code" copies without opening the window; clipboard
      clears after 60 s; the code is hidden while the session is locked.
- [ ] Magic-link mail: "Open" appears in the reader.
- [ ] Clicking the toast opens that exact message, even when the window was
      closed/hidden.
- [ ] With tabs on: "Primary + codes" mode toasts only Primary + codes.
- [ ] Quiet hours: set 10:00 PM – 6:30 AM in Settings → Notifications, send
      a regular mail (no toast; it still syncs and counts) and a code mail
      (still toasts); then set both times to Off.

## 4. Server search and tabs against the API

- [ ] Search something only Gmail would find (older than the local range);
      the "Searching all of Gmail…" banner shows and results merge.
- [ ] Gmail operators (`in:`, `label:`, `before:`, `-x`, `OR`) work.
- [ ] "Move to tab" moves the conversation in Gmail web too.
- [ ] Mark Important reflects Gmail's IMPORTANT marker.

## 5. Survive the environment

- [ ] Sleep/resume: toasts resume without waiting for backoff (log shows a
      delta right after wake; if it waits, that's ROADMAP X2).
- [ ] Wi-Fi drop for 2 min: chip shows offline, recovers to Live, no dupes.
- [ ] Restart the PC with start-at-login: tray ready, no window, badge right.

## 6. Numbers for PLAN.md (M1)

With the app running normally, record in PLAN.md's performance table:

- [ ] Cold start at login to tray: `startup ready` log line (target ≤ 1.5 s).
- [ ] Window reopen from tray (hidden mode): `window shown` line + visible
      paint (target ≤ 100 ms; destroy mode ≤ 600 ms, `fresh=true`).
- [ ] Toast latency: subject timestamp vs `toast shown` line, ~10 samples.
- [ ] Idle RAM tray-only, window-hidden, window-open:
      `powershell -ExecutionPolicy Bypass -File scripts\measure.ps1`
      (add `-Seconds 600` for a soak; app private bytes and totals are printed).
- [ ] "Copy code" → clipboard feels instant (target ≤ 200 ms).
- [ ] Local search on the full cache feels instant (≤ 50 ms for 10k messages).

## 7. macOS / Linux (experimental)

If a Mac (Apple Silicon) or a Linux desktop is available:

- [ ] The installer/.dmg/AppImage opens; Google sign-in works.
- [ ] Tray icon appears with the menu (Open / Check mail now / Quit).
- [ ] New mail shows a notification — macOS: banner (no buttons);
      Linux: "Copy code" / "Open" buttons work on GNOME/KDE.
- [ ] Quiet hours fire at the machine's local time (set a 5-minute window
      around now, send a mail, then reset).
- [ ] Full sync, search and the reader work; toasts arrive after sleep.

When everything passes, update STATUS.md (drop the "untested live" tags) and
PLAN.md (fill the Measured column), and close the corresponding items in the
Phase 1/2 checklists.
