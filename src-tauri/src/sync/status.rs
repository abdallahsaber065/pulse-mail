//! Shared, cheaply readable sync status, pushed to the UI as an event.

use serde::Serialize;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub running: bool,
    pub imap_connected: bool,
    pub syncing: bool,
    pub last_sync_at: Option<i64>,
    /// Friendly, non-technical message for the UI.
    pub error: Option<String>,
}

pub struct StatusCell {
    inner: Mutex<SyncStatus>,
    app: Option<AppHandle>,
}

impl StatusCell {
    pub fn new(app: Option<AppHandle>) -> Self {
        Self {
            inner: Mutex::new(SyncStatus::default()),
            app,
        }
    }

    pub fn get(&self) -> SyncStatus {
        self.inner.lock().unwrap().clone()
    }

    pub fn update(&self, f: impl FnOnce(&mut SyncStatus)) {
        let snapshot = {
            let mut s = self.inner.lock().unwrap();
            let before = s.clone();
            f(&mut s);
            if *s == before {
                return;
            }
            s.clone()
        };
        if let Some(app) = &self.app {
            let _ = app.emit("sync-status", snapshot);
        }
    }

    pub fn set_imap_connected(&self, v: bool) {
        self.update(|s| s.imap_connected = v);
    }

    pub fn imap_connected(&self) -> bool {
        self.inner.lock().unwrap().imap_connected
    }
}
