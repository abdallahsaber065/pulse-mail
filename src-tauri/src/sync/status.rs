//! Shared sync status + activity log, pushed to the UI as events.
//!
//! `sync-status` carries the whole (small) status; progress updates are
//! throttled so a 500-message backfill doesn't flood IPC. `sync-activity`
//! carries one human-readable activity entry.

use serde::Serialize;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

const ACTIVITY_MAX: usize = 60;
const PROGRESS_EMIT_EVERY: Duration = Duration::from_millis(120);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// What is happening, user-facing ("Getting your newest mail").
    pub label: String,
    pub done: u32,
    pub total: u32,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    /// Account this status belongs to (one cell per account).
    pub account_id: i64,
    pub email: String,
    pub running: bool,
    pub imap_connected: bool,
    pub syncing: bool,
    /// The current pass was asked for by the user (or is the first sync), so
    /// the UI may show it. Background checks (push, poll, retries) are quiet.
    pub foreground: bool,
    pub last_sync_at: Option<i64>,
    /// Friendly, non-technical message for the UI.
    pub error: Option<String>,
    /// Foreground work (first page, delta, bodies).
    pub progress: Option<Progress>,
    /// Background backfill of older mail.
    pub backfill: Option<Progress>,
    /// History download (H1): always present once the account is known.
    pub history: Option<History>,
}

/// Full-history download state for the onboarding progress screen, the
/// settings account page and the chip popover.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct History {
    /// "setup" | "pending" | "running" | "paused" | "done"
    pub state: String,
    /// "30d" | "90d" | "1y" | "all"
    pub window: String,
    /// "list" (message list) | "bodies" (offline copies) | "" when idle
    pub phase: String,
    pub done: u32,
    pub total: u32,
    /// Messages per second over this session (0 = unknown).
    pub rate: f32,
    /// Seconds left at the current rate (None = unknown).
    pub eta_secs: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Activity {
    pub at: i64,
    /// Account email when several accounts exist.
    pub account: String,
    /// "info" | "success" | "warn" | "error"
    pub level: &'static str,
    pub text: String,
}

struct Inner {
    status: SyncStatus,
    activity: VecDeque<Activity>,
    last_progress_emit: Option<Instant>,
}

pub struct StatusCell {
    inner: Mutex<Inner>,
    app: Option<AppHandle>,
}

impl StatusCell {
    pub fn new(app: Option<AppHandle>) -> Self {
        Self::for_account(app, 0, "")
    }

    pub fn for_account(app: Option<AppHandle>, account_id: i64, email: &str) -> Self {
        Self {
            inner: Mutex::new(Inner {
                status: SyncStatus {
                    account_id,
                    email: email.to_string(),
                    ..Default::default()
                },
                activity: VecDeque::new(),
                last_progress_emit: None,
            }),
            app,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A panic elsewhere must not poison status reporting forever.
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn get(&self) -> SyncStatus {
        self.lock().status.clone()
    }

    pub fn activity(&self) -> Vec<Activity> {
        self.lock().activity.iter().cloned().collect()
    }

    /// Apply a change; emits `sync-status` if anything changed.
    pub fn update(&self, f: impl FnOnce(&mut SyncStatus)) {
        let snapshot = {
            let mut inner = self.lock();
            let before = inner.status.clone();
            f(&mut inner.status);
            if inner.status == before {
                return;
            }
            inner.last_progress_emit = Some(Instant::now());
            inner.status.clone()
        };
        self.emit("sync-status", snapshot);
    }

    /// Progress counters: emitted at most every 120 ms, plus always on the
    /// first and last step so the UI never shows a stale number.
    pub fn progress(&self, backfill: bool, label: &str, done: u32, total: u32) {
        let snapshot = {
            let mut inner = self.lock();
            let p = Progress {
                label: label.to_string(),
                done,
                total,
            };
            let slot = if backfill {
                &mut inner.status.backfill
            } else {
                &mut inner.status.progress
            };
            let label_changed = slot.as_ref().is_none_or(|old| old.label != p.label);
            *slot = Some(p);
            let due = inner
                .last_progress_emit
                .is_none_or(|t| t.elapsed() >= PROGRESS_EMIT_EVERY);
            if !(label_changed || done == 0 || done >= total || due) {
                return;
            }
            inner.last_progress_emit = Some(Instant::now());
            inner.status.clone()
        };
        self.emit("sync-status", snapshot);
    }

    pub fn clear_progress(&self, backfill: bool) {
        self.update(|s| {
            if backfill {
                s.backfill = None;
            } else {
                s.progress = None;
            }
        });
    }

    pub fn set_imap_connected(&self, v: bool) {
        self.update(|s| s.imap_connected = v);
    }

    pub fn imap_connected(&self) -> bool {
        self.lock().status.imap_connected
    }

    /// History counters, throttled like `progress`.
    pub fn history(&self, h: History) {
        let snapshot = {
            let mut inner = self.lock();
            let important = inner.status.history.as_ref().is_none_or(|old| {
                old.state != h.state || old.phase != h.phase || old.total != h.total || h.done >= h.total
            });
            if inner.status.history.as_ref() == Some(&h) {
                return;
            }
            inner.status.history = Some(h);
            let due = inner
                .last_progress_emit
                .is_none_or(|t| t.elapsed() >= PROGRESS_EMIT_EVERY);
            if !(important || due) {
                return;
            }
            inner.last_progress_emit = Some(Instant::now());
            inner.status.clone()
        };
        self.emit("sync-status", snapshot);
    }

    pub fn log(&self, level: &'static str, text: impl Into<String>) {
        let mut inner = self.lock();
        let entry = Activity {
            at: super::now_ms(),
            account: inner.status.email.clone(),
            level,
            text: text.into(),
        };
        {
            if inner.activity.len() >= ACTIVITY_MAX {
                inner.activity.pop_front();
            }
            inner.activity.push_back(entry.clone());
        }
        drop(inner);
        self.emit("sync-activity", entry);
    }

    fn emit<T: Serialize + Clone>(&self, event: &str, payload: T) {
        if let Some(app) = &self.app {
            let _ = app.emit(event, payload);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_and_activity_without_app() {
        let s = StatusCell::new(None);
        s.progress(false, "Getting your newest mail", 0, 50);
        s.progress(false, "Getting your newest mail", 50, 50);
        assert_eq!(s.get().progress.unwrap().done, 50);
        s.clear_progress(false);
        assert!(s.get().progress.is_none());
        for i in 0..100 {
            s.log("info", format!("entry {i}"));
        }
        let a = s.activity();
        assert_eq!(a.len(), ACTIVITY_MAX);
        assert_eq!(a.last().unwrap().text, "entry 99");
    }

    #[test]
    fn history_updates_are_stored() {
        let s = StatusCell::for_account(None, 7, "a@example.com");
        let h = History {
            state: "running".into(),
            window: "1y".into(),
            phase: "list".into(),
            done: 10,
            total: 100,
            rate: 5.0,
            eta_secs: Some(18),
        };
        s.history(h.clone());
        assert_eq!(s.get().history, Some(h));
        assert_eq!(s.get().account_id, 7);
        s.log("info", "x");
        assert_eq!(s.activity()[0].account, "a@example.com");
    }
}
