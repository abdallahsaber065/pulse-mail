//! Sync engine.
//!
//! - IMAP IDLE (imap_idle.rs) rings a doorbell within ~1 s of new mail.
//! - Each ring (or a fallback timer) runs a Gmail `history.list` delta from
//!   the saved historyId and caches the changes in SQLite.
//! - An unknown/expired historyId (HTTP 404) triggers a bounded full resync.
//! - New unread INBOX messages raise a toast (with "Copy code" for OTPs).

pub mod gmail;
pub mod imap_idle;
pub mod mime;
pub mod status;

use crate::auth::TokenManager;
use crate::auth::oauth::AuthError;
use crate::notify::{self, NewMailToast};
use crate::store::{LabelRow, Store};
use futures::StreamExt;
use gmail::{Gmail, GmailError, HistoryDelta};
use status::StatusCell;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;

/// How many recent messages a first/full sync caches.
const FULL_SYNC_MESSAGES: usize = 300;
const FULL_SYNC_QUERY: &str = "newer_than:60d -in:spam -in:trash";
const FETCH_CONCURRENCY: usize = 6;
/// Fallback polling when IMAP is down / healthy.
const POLL_IMAP_DOWN: Duration = Duration::from_secs(60);
const POLL_IMAP_UP: Duration = Duration::from_secs(15 * 60);
/// Gmail's history can trail the IMAP push by a moment.
const EMPTY_WAKE_RETRIES: u32 = 2;
const EMPTY_WAKE_DELAY: Duration = Duration::from_millis(1500);
/// Never notify for mail older than this (e.g. after a long offline period).
const NOTIFY_MAX_AGE_MS: i64 = 15 * 60 * 1000;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Gmail(#[from] GmailError),
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
}

impl SyncError {
    fn friendly(&self) -> String {
        match self {
            SyncError::Auth(AuthError::InvalidGrant) => "Your Google sign-in expired. Please sign in again.".into(),
            SyncError::Gmail(GmailError::Network(_)) | SyncError::Auth(AuthError::Network(_)) => {
                "You're offline. New mail will appear when the connection is back.".into()
            }
            SyncError::Gmail(GmailError::RateLimited) => "Gmail asked us to slow down. Retrying shortly.".into(),
            _ => "Couldn't refresh mail. Retrying shortly.".into(),
        }
    }
}

/// Handle to a running engine; dropping it does not stop it, call `stop`.
pub struct SyncController {
    pub wake: Arc<Notify>,
    tasks: Vec<tauri::async_runtime::JoinHandle<()>>,
}

impl SyncController {
    pub fn sync_now(&self) {
        self.wake.notify_one();
    }
    pub fn stop(self) {
        for t in self.tasks {
            t.abort();
        }
    }
}

pub fn spawn(
    app: AppHandle,
    store: Arc<Mutex<Store>>,
    tokens: Arc<TokenManager>,
    http: reqwest::Client,
    account_id: i64,
    status: Arc<StatusCell>,
) -> SyncController {
    let wake = Arc::new(Notify::new());
    let engine = Engine {
        app,
        store,
        gmail: Gmail::new(http),
        tokens: tokens.clone(),
        account_id,
        status: status.clone(),
    };
    status.update(|s| {
        s.running = true;
        s.error = None;
    });
    let w = wake.clone();
    let main = tauri::async_runtime::spawn(async move { engine.run(w).await });
    let w = wake.clone();
    let idle = tauri::async_runtime::spawn(async move { imap_idle::run(tokens, w, status).await });
    SyncController {
        wake,
        tasks: vec![main, idle],
    }
}

struct Engine {
    app: AppHandle,
    store: Arc<Mutex<Store>>,
    gmail: Gmail,
    tokens: Arc<TokenManager>,
    account_id: i64,
    status: Arc<StatusCell>,
}

impl Engine {
    async fn run(self, wake: Arc<Notify>) {
        let mut woke_by_push = false;
        loop {
            let mut attempts = 0;
            loop {
                let added = self.run_once().await;
                if !(woke_by_push && added == Some(0) && attempts < EMPTY_WAKE_RETRIES) {
                    break;
                }
                attempts += 1;
                tokio::time::sleep(EMPTY_WAKE_DELAY).await;
            }
            let poll = if self.status.imap_connected() {
                POLL_IMAP_UP
            } else {
                POLL_IMAP_DOWN
            };
            woke_by_push = tokio::select! {
                _ = wake.notified() => true,
                _ = tokio::time::sleep(poll) => false,
            };
        }
    }

    /// One sync pass. Returns the number of newly added messages, or None on error.
    async fn run_once(&self) -> Option<usize> {
        self.status.update(|s| s.syncing = true);
        let result = match self.sync().await {
            Err(SyncError::Gmail(GmailError::Unauthorized)) => {
                self.tokens.invalidate().await;
                self.sync().await
            }
            r => r,
        };
        let now = now_ms();
        match result {
            Ok(added) => {
                self.status.update(|s| {
                    s.syncing = false;
                    s.error = None;
                    s.last_sync_at = Some(now);
                });
                Some(added)
            }
            Err(e) => {
                tracing::warn!(error = %e, "sync failed");
                let msg = e.friendly();
                self.status.update(|s| {
                    s.syncing = false;
                    s.error = Some(msg);
                });
                None
            }
        }
    }

    async fn sync(&self) -> Result<usize, SyncError> {
        let history_id = self.db(|s| s.primary_account())?.and_then(|a| a.history_id);
        match history_id {
            None => self.full_sync().await,
            Some(h) => match self.delta_sync(&h).await {
                Err(SyncError::Gmail(GmailError::HistoryExpired)) => {
                    tracing::info!("history id expired, running full resync");
                    self.db(|s| s.clear_history_id(self.account_id))?;
                    self.full_sync().await
                }
                r => r,
            },
        }
    }

    /// Bounded initial/full sync. Never raises notifications.
    async fn full_sync(&self) -> Result<usize, SyncError> {
        let token = self.tokens.access_token().await?;
        // Snapshot the history id *before* listing so nothing slips between.
        let profile = self.gmail.profile(&token).await?;
        self.sync_labels(&token).await?;
        let refs = self
            .gmail
            .list_message_ids(&token, FULL_SYNC_QUERY, FULL_SYNC_MESSAGES)
            .await?;
        let mut stream = futures::stream::iter(refs)
            .map(|r| {
                let token = token.clone();
                async move { self.gmail.get_raw(&token, &r.id).await }
            })
            .buffer_unordered(FETCH_CONCURRENCY);
        let mut count = 0;
        while let Some(res) = stream.next().await {
            match res {
                Ok(raw) => {
                    let bytes = raw.decode_raw().unwrap_or_default();
                    let m = mime::to_new_message(&raw, &bytes);
                    self.db(|s| s.upsert_message(self.account_id, &m))?;
                    count += 1;
                }
                Err(GmailError::Status(404)) => {} // deleted meanwhile
                Err(e) => return Err(e.into()),
            }
        }
        self.db(|s| s.set_history_id(self.account_id, &profile.history_id, now_ms()))?;
        tracing::info!(count, "full sync done");
        let _ = self.app.emit("mail-updated", ());
        Ok(0)
    }

    async fn delta_sync(&self, start: &str) -> Result<usize, SyncError> {
        let token = self.tokens.access_token().await?;
        let (records, latest) = self.gmail.history_since(&token, start).await?;
        let delta = HistoryDelta::from_records(&records);
        let changed = !(delta.added.is_empty() && delta.deleted.is_empty() && delta.relabeled.is_empty());

        for id in &delta.deleted {
            self.db(|s| s.delete_message(self.account_id, id))?;
        }
        for (id, labels) in &delta.relabeled {
            self.db(|s| s.set_message_labels(self.account_id, id, labels))?;
        }

        let mut added = 0;
        let mut fetched = futures::stream::iter(delta.added.clone())
            .map(|id| {
                let token = token.clone();
                async move { self.gmail.get_raw(&token, &id).await }
            })
            .buffered(FETCH_CONCURRENCY); // keep arrival order for notifications
        while let Some(res) = fetched.next().await {
            let raw = match res {
                Ok(r) => r,
                Err(GmailError::Status(404)) => continue,
                Err(e) => return Err(e.into()),
            };
            let bytes = raw.decode_raw().unwrap_or_default();
            let m = mime::to_new_message(&raw, &bytes);
            let existed = self.db(|s| s.has_message(self.account_id, &m.id))?;
            self.db(|s| s.upsert_message(self.account_id, &m))?;
            added += 1;

            let is_new_inbox = m.label_ids.iter().any(|l| l == "INBOX") && m.label_ids.iter().any(|l| l == "UNREAD");
            if !existed && is_new_inbox && now_ms() - m.internal_date < NOTIFY_MAX_AGE_MS {
                notify::show_new_mail(
                    &self.app,
                    NewMailToast {
                        message_id: m.id.clone(),
                        thread_id: m.thread_id.clone(),
                        from: if m.from_name.is_empty() {
                            m.from_email.clone()
                        } else {
                            m.from_name.clone()
                        },
                        subject: m.subject.clone(),
                        snippet: m.snippet.clone(),
                        otp: m.otp_code.clone(),
                    },
                );
            }
        }

        if !delta.added.is_empty() {
            // Labels' message counts drift; refresh them cheaply after new mail.
            if let Err(e) = self.sync_labels(&token).await {
                tracing::debug!(error = %e, "label refresh failed");
            }
        }
        self.db(|s| s.set_history_id(self.account_id, &latest, now_ms()))?;
        if changed {
            let _ = self.app.emit("mail-updated", ());
        }
        Ok(added)
    }

    async fn sync_labels(&self, token: &crate::auth::secret::Secret) -> Result<(), SyncError> {
        let labels = self.gmail.labels(token).await?;
        let rows: Vec<LabelRow> = labels
            .into_iter()
            .map(|l| LabelRow {
                id: l.id,
                name: l.name,
                kind: if l.kind.eq_ignore_ascii_case("system") {
                    "system".into()
                } else {
                    "user".into()
                },
                unread: l.messages_unread,
                total: l.messages_total,
            })
            .collect();
        self.db(|s| s.replace_labels(self.account_id, &rows))?;
        Ok(())
    }

    fn db<T>(&self, f: impl FnOnce(&mut Store) -> rusqlite::Result<T>) -> Result<T, SyncError> {
        let mut guard = self.store.lock().unwrap();
        Ok(f(&mut guard)?)
    }
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}
