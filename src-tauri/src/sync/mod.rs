//! Sync engine.
//!
//! - IMAP IDLE (imap_idle.rs) rings a doorbell within ~1 s of new mail.
//! - Each ring (or a fallback timer) runs a Gmail `history.list` delta from
//!   the saved historyId and caches the changes in SQLite.
//! - First sync is metadata-first: the 50 newest INBOX messages are fetched
//!   with `format=metadata` (small) and streamed to the UI as they land; the
//!   historyId is saved right after, so live mail works immediately. Then a
//!   background task prefetches bodies of the newest messages and backfills
//!   older mail. Other bodies load lazily when a thread is opened.
//! - An unknown/expired historyId (HTTP 404) triggers a fresh first sync.
//! - New unread INBOX messages raise a toast (with "Copy code" for OTPs).
//! - A panic anywhere in a pass is caught, logged and reported; the loop
//!   keeps running.

pub mod gmail;
pub mod imap_idle;
pub mod mime;
pub mod status;

use crate::auth::TokenManager;
use crate::auth::oauth::AuthError;
use crate::auth::secret::Secret;
use crate::notify::{self, NewMailToast};
use crate::store::{LabelRow, Store};
use futures::{FutureExt, StreamExt};
use gmail::{Gmail, GmailError, HistoryDelta};
use status::StatusCell;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;

/// First page: what the user sees within a couple of seconds.
const FIRST_PAGE: usize = 50;
/// Background backfill window.
const BACKFILL_MAX: usize = 500;
const BACKFILL_QUERY: &str = "newer_than:90d -in:spam -in:trash";
/// Bodies prefetched for the newest INBOX messages.
const PREFETCH_BODIES: i64 = 12;
/// Gmail allows 250 quota units/s per user; messages.get costs 5. Ten
/// in-flight requests (~0.6 s each from here) stays under that; 429s are
/// retried. Only used for the few batch parts that come back throttled.
const META_CONCURRENCY: usize = 10;
const RAW_CONCURRENCY: usize = 6;
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
    pub fn friendly(&self) -> String {
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

/// Handle to a running engine.
pub struct SyncController {
    pub wake: Arc<Notify>,
    engine: Arc<Engine>,
    tasks: Vec<tauri::async_runtime::JoinHandle<()>>,
}

impl SyncController {
    pub fn sync_now(&self) {
        self.wake.notify_one();
    }

    pub fn engine(&self) -> Arc<Engine> {
        self.engine.clone()
    }

    pub fn stop(self) {
        self.engine.stopped.store(true, Ordering::Relaxed);
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
    let engine = Arc::new(Engine {
        app,
        store,
        gmail: Gmail::new(http),
        tokens: tokens.clone(),
        account_id,
        status: status.clone(),
        stopped: AtomicBool::new(false),
        background_running: AtomicBool::new(false),
    });
    status.update(|s| {
        s.running = true;
        s.error = None;
    });
    let (e, w) = (engine.clone(), wake.clone());
    let main = tauri::async_runtime::spawn(async move { e.run(w).await });
    let w = wake.clone();
    let idle = tauri::async_runtime::spawn(async move { imap_idle::run(tokens, w, status).await });
    SyncController {
        wake,
        engine,
        tasks: vec![main, idle],
    }
}

pub struct Engine {
    app: AppHandle,
    store: Arc<Mutex<Store>>,
    gmail: Gmail,
    tokens: Arc<TokenManager>,
    account_id: i64,
    status: Arc<StatusCell>,
    stopped: AtomicBool,
    background_running: AtomicBool,
}

impl Engine {
    async fn run(self: Arc<Self>, wake: Arc<Notify>) {
        // Resume work an earlier run didn't finish (backfill, prefetch).
        if matches!(self.db(|s| s.primary_account()), Ok(Some(a)) if a.history_id.is_some()) {
            self.spawn_background();
        }
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

    /// One sync pass. Returns the number of newly added messages, or None on
    /// error. Never panics: a panic inside is caught and reported.
    async fn run_once(self: &Arc<Self>) -> Option<usize> {
        self.status.update(|s| s.syncing = true);
        let pass = async {
            match self.sync().await {
                Err(SyncError::Gmail(GmailError::Unauthorized)) => {
                    self.tokens.invalidate().await;
                    self.sync().await
                }
                r => r,
            }
        };
        let result = AssertUnwindSafe(pass).catch_unwind().await;
        let now = now_ms();
        match result {
            Ok(Ok(added)) => {
                self.status.update(|s| {
                    s.syncing = false;
                    s.error = None;
                    s.progress = None;
                    s.last_sync_at = Some(now);
                });
                Some(added)
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "sync failed");
                let msg = e.friendly();
                self.status.log("error", msg.clone());
                self.status.update(|s| {
                    s.syncing = false;
                    s.progress = None;
                    s.error = Some(msg);
                });
                None
            }
            Err(panic) => {
                tracing::error!(panic = %panic_text(&panic), "sync pass panicked; will retry");
                let msg = "Something went wrong while syncing. Retrying shortly.".to_string();
                self.status.log("error", msg.clone());
                self.status.update(|s| {
                    s.syncing = false;
                    s.progress = None;
                    s.error = Some(msg);
                });
                None
            }
        }
    }

    async fn sync(self: &Arc<Self>) -> Result<usize, SyncError> {
        let history_id = self.db(|s| s.primary_account())?.and_then(|a| a.history_id);
        match history_id {
            None => self.first_sync().await,
            Some(h) => match self.delta_sync(&h).await {
                Err(SyncError::Gmail(GmailError::HistoryExpired)) => {
                    tracing::info!("history id expired, running a fresh first sync");
                    self.status.log("warn", "Been offline a while: refreshing your mail");
                    self.db(|s| s.clear_history_id(self.account_id))?;
                    self.db(|s| s.set_setting(&self.backfill_key(), "0"))?;
                    self.first_sync().await
                }
                r => r,
            },
        }
    }

    /// Fast first sync: newest INBOX page via metadata, streamed to the UI.
    /// Never raises notifications.
    async fn first_sync(self: &Arc<Self>) -> Result<usize, SyncError> {
        let t0 = Instant::now();
        const LABEL: &str = "Getting your newest mail";
        self.status.progress(false, LABEL, 0, FIRST_PAGE as u32);
        let token = self.tokens.access_token().await?;
        // Snapshot the history id *before* listing so nothing slips between.
        let (profile, refs) = tokio::try_join!(
            self.gmail.profile(&token),
            self.gmail.list_message_ids(&token, Some("INBOX"), None, FIRST_PAGE),
        )?;
        let ids: Vec<String> = refs.into_iter().map(|r| r.id).collect();
        let n = self.fetch_metadata(&token, ids, false, LABEL).await?;
        let secs = t0.elapsed().as_secs_f64();
        tracing::info!(count = n, ms = t0.elapsed().as_millis() as u64, "first page ready");
        self.status
            .log("success", format!("Inbox ready: {n} newest messages in {secs:.1} s"));

        if let Err(e) = self.sync_labels(&token).await {
            tracing::warn!(error = %e, "label sync failed");
        }
        self.db(|s| s.set_history_id(self.account_id, &profile.history_id, now_ms()))?;
        let _ = self.app.emit("mail-updated", ());
        self.spawn_background();
        Ok(0)
    }

    /// Prefetch bodies of the newest messages, then backfill older mail.
    /// Runs alongside the main loop so live mail is never blocked.
    fn spawn_background(self: &Arc<Self>) {
        if self.background_running.swap(true, Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            let r = AssertUnwindSafe(me.background()).catch_unwind().await;
            match r {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    tracing::warn!(error = %e, "background sync stopped");
                    me.status
                        .log("warn", "Paused downloading older mail. It will resume automatically.");
                }
                Err(p) => tracing::error!(panic = %panic_text(&p), "background sync panicked"),
            }
            me.status.clear_progress(true);
            me.background_running.store(false, Ordering::SeqCst);
        });
    }

    async fn background(&self) -> Result<(), SyncError> {
        let token = self.tokens.access_token().await?;
        let ids = self.db(|s| s.inbox_ids_without_body(self.account_id, PREFETCH_BODIES))?;
        if !ids.is_empty() {
            self.fetch_bodies(&token, ids, true, "Preparing your newest messages")
                .await?;
        }
        if self.db(|s| s.get_setting(&self.backfill_key()))?.as_deref() == Some("1") {
            return Ok(());
        }
        let t0 = Instant::now();
        const LABEL: &str = "Downloading older mail";
        self.status.progress(true, LABEL, 0, 0);
        let refs = self
            .gmail
            .list_message_ids(&token, None, Some(BACKFILL_QUERY), BACKFILL_MAX)
            .await?;
        let mut missing = Vec::new();
        for r in refs {
            if !self.db(|s| s.has_message(self.account_id, &r.id))? {
                missing.push(r.id);
            }
        }
        if !missing.is_empty() {
            self.status.log(
                "info",
                format!("Downloading {} older messages in the background", missing.len()),
            );
        }
        let n = self.fetch_metadata(&token, missing, true, LABEL).await?;
        if self.stopped.load(Ordering::Relaxed) {
            return Ok(());
        }
        self.db(|s| s.set_setting(&self.backfill_key(), "1"))?;
        let total = self.db(|s| s.message_count(self.account_id))?;
        tracing::info!(count = n, ms = t0.elapsed().as_millis() as u64, "backfill done");
        self.status.log(
            "success",
            format!("All recent mail is on this PC ({total} messages, last 90 days)"),
        );
        Ok(())
    }

    /// Fetch `format=metadata` for `ids` (in order, bounded concurrency),
    /// upsert each, report progress and refresh the UI as rows land.
    async fn fetch_metadata(
        &self,
        token: &Secret,
        ids: Vec<String>,
        backfill: bool,
        label: &str,
    ) -> Result<usize, SyncError> {
        let total = ids.len() as u32;
        if total == 0 {
            return Ok(0);
        }
        self.status.progress(backfill, label, 0, total);
        // One batch request per 50 ids (a single round trip, ~0.6 s for 50 vs
        // ~3 s for 50 single gets at 10 in flight). Each batch costs 250 quota
        // units = Gmail's per-second budget, so batches are paced >= 1 s apart.
        let (mut done, mut stored) = (0u32, 0usize);
        for chunk in ids.chunks(gmail::BATCH_MAX) {
            if self.stopped.load(Ordering::Relaxed) {
                break;
            }
            let started = Instant::now();
            let results = match self.gmail.get_metadata_batch(token, chunk).await {
                Ok(r) => r,
                Err(GmailError::Unauthorized) => return Err(GmailError::Unauthorized.into()),
                Err(e) => {
                    tracing::warn!(error = %e, "batch failed; falling back to single gets");
                    chunk.iter().map(|_| Err(0)).collect()
                }
            };
            // Throttled / failed parts: retry one by one (with backoff).
            let mut retry = Vec::new();
            for (id, r) in chunk.iter().zip(results) {
                match r {
                    Ok(meta) => {
                        let m = mime::from_metadata(&meta);
                        self.db(|s| s.upsert_metadata(self.account_id, &m))?;
                        stored += 1;
                        done += 1;
                    }
                    Err(404) => done += 1, // deleted meanwhile
                    Err(_) => retry.push(id.clone()),
                }
            }
            self.status.progress(backfill, label, done, total);
            if !retry.is_empty() {
                let _ = self.app.emit("mail-updated", ());
                let mut stream = futures::stream::iter(retry)
                    .map(|id| {
                        let token = token.clone();
                        async move { self.gmail.get_metadata(&token, &id).await }
                    })
                    .buffered(META_CONCURRENCY);
                while let Some(res) = stream.next().await {
                    match res {
                        Ok(meta) => {
                            let m = mime::from_metadata(&meta);
                            self.db(|s| s.upsert_metadata(self.account_id, &m))?;
                            stored += 1;
                        }
                        Err(GmailError::Status(404)) => {}
                        Err(e) => return Err(e.into()),
                    }
                    done += 1;
                    self.status.progress(backfill, label, done, total);
                }
            }
            let _ = self.app.emit("mail-updated", ());
            if done < total {
                tokio::time::sleep(Duration::from_secs(1).saturating_sub(started.elapsed())).await;
            }
        }
        let _ = self.app.emit("mail-updated", ());
        self.status.clear_progress(backfill);
        Ok(stored)
    }

    async fn fetch_bodies(
        &self,
        token: &Secret,
        ids: Vec<String>,
        background: bool,
        label: &str,
    ) -> Result<usize, SyncError> {
        let total = ids.len() as u32;
        self.status.progress(background, label, 0, total);
        let mut stream = futures::stream::iter(ids)
            .map(|id| {
                let token = token.clone();
                async move { self.gmail.get_raw(&token, &id).await }
            })
            .buffer_unordered(RAW_CONCURRENCY);
        let mut done = 0u32;
        while let Some(res) = stream.next().await {
            match res {
                Ok(raw) => {
                    let bytes = raw.decode_raw().unwrap_or_default();
                    let m = mime::to_new_message(&raw, &bytes);
                    self.db(|s| s.upsert_message(self.account_id, &m))?;
                }
                Err(GmailError::Status(404)) => {}
                Err(e) => return Err(e.into()),
            }
            done += 1;
            self.status.progress(background, label, done, total);
        }
        self.status.clear_progress(background);
        Ok(done as usize)
    }

    /// Download any missing bodies of a thread (called when it's opened).
    pub async fn load_thread_bodies(&self, thread_id: &str) -> Result<usize, SyncError> {
        let ids = self.db(|s| s.ids_without_body(self.account_id, thread_id))?;
        if ids.is_empty() {
            return Ok(0);
        }
        let token = self.tokens.access_token().await?;
        let mut stream = futures::stream::iter(ids)
            .map(|id| {
                let token = token.clone();
                async move { self.gmail.get_raw(&token, &id).await }
            })
            .buffer_unordered(RAW_CONCURRENCY);
        let mut n = 0;
        while let Some(res) = stream.next().await {
            match res {
                Ok(raw) => {
                    let bytes = raw.decode_raw().unwrap_or_default();
                    self.db(|s| s.upsert_message(self.account_id, &mime::to_new_message(&raw, &bytes)))?;
                    n += 1;
                }
                Err(GmailError::Status(404)) => {}
                Err(GmailError::Unauthorized) => {
                    self.tokens.invalidate().await;
                    return Err(GmailError::Unauthorized.into());
                }
                Err(e) => return Err(e.into()),
            }
        }
        Ok(n)
    }

    /// Archive / star / read: local first (instant UI), then Gmail. On
    /// failure the local change is rolled back.
    pub async fn modify_thread(&self, thread_id: &str, add: &[String], remove: &[String]) -> Result<(), SyncError> {
        self.db(|s| s.modify_thread_labels(self.account_id, thread_id, add, remove))?;
        let _ = self.app.emit("mail-updated", ());
        let result = async {
            let token = self.tokens.access_token().await?;
            match self.gmail.modify_thread(&token, thread_id, add, remove).await {
                Err(GmailError::Unauthorized) => {
                    self.tokens.invalidate().await;
                    let token = self.tokens.access_token().await?;
                    self.gmail.modify_thread(&token, thread_id, add, remove).await?;
                }
                r => r?,
            }
            Ok::<_, SyncError>(())
        }
        .await;
        if let Err(e) = &result {
            tracing::warn!(error = %e, "modify thread failed; rolling back");
            self.db(|s| s.modify_thread_labels(self.account_id, thread_id, remove, add))?;
            let _ = self.app.emit("mail-updated", ());
        }
        result
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

        let total = delta.added.len() as u32;
        let label = if total == 1 {
            "Getting 1 new message".to_string()
        } else {
            format!("Getting {total} new messages")
        };
        if total > 0 {
            self.status.progress(false, &label, 0, total);
        }
        let notify_pref = self
            .db(|s| s.get_setting("notifications"))?
            .unwrap_or_else(|| "all".into());
        let mut added = 0;
        let mut fetched = futures::stream::iter(delta.added.clone())
            .map(|id| {
                let token = token.clone();
                async move { self.gmail.get_raw(&token, &id).await }
            })
            .buffered(RAW_CONCURRENCY); // keep arrival order for notifications
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
            self.status.progress(false, &label, added as u32, total);

            let is_new_inbox = m.label_ids.iter().any(|l| l == "INBOX") && m.label_ids.iter().any(|l| l == "UNREAD");
            let wanted = match notify_pref.as_str() {
                "off" => false,
                "otp" => m.otp_code.is_some(),
                _ => true,
            };
            if wanted && !existed && is_new_inbox && now_ms() - m.internal_date < NOTIFY_MAX_AGE_MS {
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
        if added > 0 {
            self.status.log(
                "info",
                if added == 1 {
                    "1 new message".to_string()
                } else {
                    format!("{added} new messages")
                },
            );
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

    async fn sync_labels(&self, token: &Secret) -> Result<(), SyncError> {
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

    fn backfill_key(&self) -> String {
        format!("backfill_done:{}", self.account_id)
    }

    fn db<T>(&self, f: impl FnOnce(&mut Store) -> rusqlite::Result<T>) -> Result<T, SyncError> {
        // Recover from a poisoned lock: SQLite state is transactional.
        let mut guard = self.store.lock().unwrap_or_else(|e| e.into_inner());
        Ok(f(&mut guard)?)
    }
}

pub(crate) fn panic_text(p: &Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| p.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".into())
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}
