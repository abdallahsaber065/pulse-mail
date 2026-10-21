//! Sync engine.
//!
//! - IMAP IDLE (imap_idle.rs) rings a doorbell within ~1 s of new mail.
//! - Each ring (or a fallback timer) runs a Gmail `history.list` delta from
//!   the saved historyId and caches the changes in SQLite.
//! - First sync is metadata-first: the 50 newest INBOX messages are fetched
//!   with `format=metadata` (small) and streamed to the UI as they land; the
//!   historyId is saved right after, so live mail works immediately. Then a
//!   background task prefetches bodies of the newest messages and runs the
//!   history download (H1): the message list for the account's chosen window
//!   (30 days .. everything), page by page with a saved cursor so it resumes
//!   after a restart, then offline copies of bodies newer than `offline_days`
//!   within the storage budget. It can be paused and resumed. Other bodies
//!   load lazily when a thread is opened.
//! - One engine per account; all accounts sync and notify.
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
use crate::store::{Account, LabelRow, Store};
use futures::{FutureExt, StreamExt};
use gmail::{Gmail, GmailError, HistoryDelta};
use status::{History, StatusCell};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::Notify;

/// First page: what the user sees within a couple of seconds.
const FIRST_PAGE: usize = 50;
/// Ids per history-download page (Gmail's maximum).
const HISTORY_PAGE: u32 = 500;
/// Offline copies are fetched in chunks of this many ids.
const BODY_CHUNK: i64 = 200;
/// Bodies prefetched for the newest INBOX messages.
const PREFETCH_BODIES: i64 = 12;
/// Gmail allows 250 quota units/s per user; messages.get costs 5. Ten
/// in-flight requests (~0.6 s each from here) stays under that; 429s are
/// retried. Only used for the few batch parts that come back throttled.
const META_CONCURRENCY: usize = 10;
const RAW_CONCURRENCY: usize = 6;
/// A push/poll delta at least this big is shown in the UI (catch-up after
/// being offline); smaller ones are applied quietly.
const LOUD_DELTA: u32 = 25;
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
    /// Wake the engine. `manual` marks the pass as user-initiated so the UI
    /// shows it; automatic wake-ups (network back, push) stay quiet.
    pub fn sync_now(&self, manual: bool) {
        if manual {
            self.engine.manual.store(true, Ordering::SeqCst);
        }
        self.wake.notify_one();
    }

    pub fn engine(&self) -> Arc<Engine> {
        self.engine.clone()
    }

    pub fn status(&self) -> Arc<StatusCell> {
        self.engine.status.clone()
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
        background_again: AtomicBool::new(false),
        paused: AtomicBool::new(false),
        hist: Mutex::new(HistSession::default()),
        manual: AtomicBool::new(false),
        failures: AtomicU32::new(0),
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

/// Counters for the running history download (rate / ETA).
#[derive(Default)]
struct HistSession {
    window: String,
    phase: &'static str,
    base: u32,
    total: u32,
    started: Option<Instant>,
    session_done: u32,
}

/// Gmail search for an account's download window.
pub fn window_query(window: &str) -> &'static str {
    match window {
        "30d" => "newer_than:30d",
        "1y" => "newer_than:1y",
        "all" => "",
        _ => "newer_than:90d",
    }
}

pub fn window_label(window: &str) -> &'static str {
    match window {
        "30d" => "the last 30 days",
        "1y" => "the last year",
        "all" => "all your mail",
        _ => "the last 90 days",
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
    /// Background work was requested while it was running: run it again.
    background_again: AtomicBool,
    /// History download paused by the user.
    paused: AtomicBool,
    hist: Mutex<HistSession>,
    /// Set by a user-initiated refresh; consumed by the next pass.
    manual: AtomicBool,
    /// Consecutive failed passes; background failures surface after two.
    failures: AtomicU32,
}

impl Engine {
    async fn run(self: Arc<Self>, wake: Arc<Notify>) {
        // Resume work an earlier run didn't finish (history download, prefetch).
        if let Ok(Some(a)) = self.db(|s| s.account(self.account_id)) {
            if a.backfill_state == "paused" {
                self.paused.store(true, Ordering::SeqCst);
            }
            self.publish_history(&a, "", 0.0, None);
            if a.history_id.is_some() {
                self.spawn_background();
            }
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
        let first = !matches!(self.db(|s| s.account(self.account_id)), Ok(Some(a)) if a.history_id.is_some());
        let foreground = self.manual.swap(false, Ordering::SeqCst) || first;
        self.status.update(|s| {
            s.syncing = true;
            s.foreground = foreground;
        });
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
                self.failures.store(0, Ordering::SeqCst);
                self.status.update(|s| {
                    s.syncing = false;
                    s.foreground = false;
                    s.error = None;
                    s.progress = None;
                    s.last_sync_at = Some(now);
                });
                Some(added)
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "sync failed");
                let msg = e.friendly();
                let fails = self.failures.fetch_add(1, Ordering::SeqCst) + 1;
                // A single failed background check is logged but not shown on
                // the chip: the next poll usually succeeds. Sign-in problems,
                // user-initiated passes and repeated failures are shown.
                let show = foreground || fails >= 2 || matches!(e, SyncError::Auth(AuthError::InvalidGrant));
                self.status.log(if show { "error" } else { "warn" }, msg.clone());
                self.status.update(|s| {
                    s.syncing = false;
                    s.foreground = false;
                    s.progress = None;
                    if show {
                        s.error = Some(msg);
                    }
                });
                None
            }
            Err(panic) => {
                tracing::error!(panic = %panic_text(&panic), "sync pass panicked; will retry");
                let msg = "Something went wrong while syncing. Retrying shortly.".to_string();
                self.failures.fetch_add(1, Ordering::SeqCst);
                self.status.log("error", msg.clone());
                self.status.update(|s| {
                    s.syncing = false;
                    s.foreground = false;
                    s.progress = None;
                    s.error = Some(msg);
                });
                None
            }
        }
    }

    async fn sync(self: &Arc<Self>) -> Result<usize, SyncError> {
        let history_id = self.db(|s| s.account(self.account_id))?.and_then(|a| a.history_id);
        match history_id {
            None => self.first_sync().await,
            Some(h) => match self.delta_sync(&h).await {
                Err(SyncError::Gmail(GmailError::HistoryExpired)) => {
                    tracing::info!("history id expired, running a fresh first sync");
                    self.status.log("warn", "Been offline a while: refreshing your mail");
                    self.db(|s| s.clear_history_id(self.account_id))?;
                    // Re-run the history download so anything missed lands
                    // (messages already here are skipped quickly).
                    let window = self.account()?.sync_window;
                    self.db(|s| s.set_account_window(self.account_id, &window))?;
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
        self.db(|s| s.set_messages_total(self.account_id, profile.messages_total))?;
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

    /// Prefetch bodies of the newest messages, then run the history
    /// download. Runs alongside the main loop so live mail is never blocked.
    /// Calling it while it runs schedules one more round.
    pub fn spawn_background(self: &Arc<Self>) {
        self.background_again.store(true, Ordering::SeqCst);
        if self.background_running.swap(true, Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        tauri::async_runtime::spawn(async move {
            while me.background_again.swap(false, Ordering::SeqCst) && !me.stopped.load(Ordering::Relaxed) {
                let r = AssertUnwindSafe(me.background()).catch_unwind().await;
                match r {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        tracing::warn!(error = %e, "background sync stopped");
                        me.status
                            .log("warn", "Paused downloading your mail. It will resume automatically.");
                    }
                    Err(p) => tracing::error!(panic = %panic_text(&p), "background sync panicked"),
                }
                me.status.clear_progress(true);
                if let Ok(a) = me.account() {
                    me.publish_history(&a, "", 0.0, None);
                }
            }
            me.background_running.store(false, Ordering::SeqCst);
        });
    }

    async fn background(self: &Arc<Self>) -> Result<(), SyncError> {
        self.hist.lock().unwrap_or_else(|e| e.into_inner()).phase = "";
        let token = self.tokens.access_token().await?;
        let ids = self.db(|s| s.inbox_ids_without_body(self.account_id, PREFETCH_BODIES))?;
        if !ids.is_empty() {
            self.fetch_bodies(&token, ids, true, "Preparing your newest messages")
                .await?;
        }
        self.history_download().await
    }

    /// Pause the history download (state is saved; resumes from the cursor).
    pub fn pause_history(&self) -> Result<(), SyncError> {
        self.paused.store(true, Ordering::SeqCst);
        self.db(|s| s.set_backfill_state(self.account_id, "paused"))?;
        let a = self.account()?;
        self.publish_history(&a, "", 0.0, None);
        self.status.log("info", "Paused downloading your mail");
        Ok(())
    }

    /// Resume (or start, after onboarding) the history download.
    pub fn resume_history(self: &Arc<Self>) -> Result<(), SyncError> {
        self.paused.store(false, Ordering::SeqCst);
        let a = self.account()?;
        if a.backfill_state != "done" {
            self.db(|s| s.set_backfill_state(self.account_id, "pending"))?;
        }
        let a = self.account()?;
        self.publish_history(&a, "", 0.0, None);
        self.spawn_background();
        Ok(())
    }

    fn account(&self) -> Result<Account, SyncError> {
        self.db(|s| s.account(self.account_id))?
            .ok_or(SyncError::Db(rusqlite::Error::QueryReturnedNoRows))
    }

    fn halted(&self) -> bool {
        self.stopped.load(Ordering::Relaxed) || self.paused.load(Ordering::Relaxed)
    }

    /// Push the account's history state to the UI.
    fn publish_history(&self, a: &Account, phase: &str, rate: f32, eta: Option<u32>) {
        let state = if self.paused.load(Ordering::Relaxed) && a.backfill_state != "done" {
            "paused".to_string()
        } else {
            a.backfill_state.clone()
        };
        self.status.history(History {
            state,
            window: a.sync_window.clone(),
            phase: phase.to_string(),
            done: a.backfill_done.max(0) as u32,
            total: a.backfill_total.max(a.backfill_done).max(0) as u32,
            rate,
            eta_secs: eta,
        });
    }

    fn hist_begin(&self, window: &str, phase: &'static str, base: u32, total: u32) {
        let mut h = self.hist.lock().unwrap_or_else(|e| e.into_inner());
        *h = HistSession {
            window: window.to_string(),
            phase,
            base,
            total,
            started: Some(Instant::now()),
            session_done: 0,
        };
        drop(h);
        self.hist_emit();
    }

    /// `n` more messages handled in the current phase.
    fn hist_tick(&self, n: u32) {
        self.hist.lock().unwrap_or_else(|e| e.into_inner()).session_done += n;
        self.hist_emit();
    }

    fn hist_set_total(&self, total: u32) {
        self.hist.lock().unwrap_or_else(|e| e.into_inner()).total = total;
    }

    fn hist_emit(&self) {
        let h = self.hist.lock().unwrap_or_else(|e| e.into_inner());
        if h.phase.is_empty() {
            return;
        }
        let done = h.base + h.session_done;
        let total = h.total.max(done);
        let secs = h.started.map(|t| t.elapsed().as_secs_f32()).unwrap_or(0.0);
        let rate = if secs >= 2.0 { h.session_done as f32 / secs } else { 0.0 };
        let eta = (rate > 0.1).then(|| ((total - done) as f32 / rate).round() as u32);
        self.status.history(History {
            state: "running".into(),
            window: h.window.clone(),
            phase: h.phase.to_string(),
            done,
            total,
            rate,
            eta_secs: eta,
        });
    }

    /// H1: download the message list for the account's window, page by page
    /// (cursor saved after each page), then offline copies of recent bodies.
    async fn history_download(self: &Arc<Self>) -> Result<(), SyncError> {
        let a = self.account()?;
        if matches!(a.backfill_state.as_str(), "setup" | "done") || self.halted() {
            return Ok(());
        }
        self.db(|s| s.set_backfill_state(self.account_id, "running"))?;
        let window = a.sync_window.clone();
        let q = window_query(&window);
        let t0 = Instant::now();
        let (mut page, mut done, mut total) = (a.backfill_page.clone(), a.backfill_done.max(0) as u32, a.backfill_total.max(0) as u32);
        if done == 0 {
            self.status
                .log("info", format!("Downloading {} to this PC", window_label(&window)));
        }
        const LABEL: &str = "Downloading your mail";
        self.hist_begin(&window, "list", done, total);
        loop {
            if self.halted() {
                return Ok(());
            }
            let p = {
                let page = page.clone();
                self.with_token(|t| {
                    let page = page.clone();
                    async move { self.gmail.list_page(&t, q, page.as_deref(), HISTORY_PAGE).await }
                })
                .await?
            };
            // Gmail's estimate is rough; "everything" uses the profile total.
            let estimate = if window == "all" { a.messages_total.max(p.estimate) } else { p.estimate };
            total = total.max(estimate.max(0) as u32).max(done + p.ids.len() as u32);
            self.hist_set_total(total);
            let page_len = p.ids.len() as u32;
            let mut missing = Vec::new();
            for id in p.ids {
                if !self.db(|s| s.has_message(self.account_id, &id))? {
                    missing.push(id);
                }
            }
            // Ids already here count straight away; fetched ones tick as they land.
            self.hist_tick(page_len - missing.len() as u32);
            let token = self.tokens.access_token().await?;
            self.fetch_metadata(&token, missing, true, LABEL).await?;
            if self.halted() {
                // The page wasn't finished: keep the old cursor so it is redone.
                return Ok(());
            }
            done += page_len;
            page = p.next;
            if page.is_none() {
                total = done;
            }
            self.db(|s| s.set_backfill_progress(self.account_id, page.as_deref(), done as i64, total as i64))?;
            if page.is_none() {
                break;
            }
        }
        tracing::info!(count = done, ms = t0.elapsed().as_millis() as u64, "history list done");

        // Offline copies of recent bodies, within the storage budget.
        let (days, budget_mb) = self.db(|s| Ok((crate::prefs::get_num(s, "offline_days"), crate::prefs::get_num(s, "storage_budget_mb"))))?;
        let since = now_ms() - days as i64 * 86_400_000;
        let pending = self.db(|s| s.ids_without_body_since(self.account_id, since, 100_000))?;
        if !pending.is_empty() {
            self.hist_begin(&window, "bodies", 0, pending.len() as u32);
            for chunk in pending.chunks(BODY_CHUNK as usize) {
                if self.halted() {
                    return Ok(());
                }
                if self.db(|s| s.db_size())? >= budget_mb as i64 * 1024 * 1024 {
                    self.status.log(
                        "warn",
                        "Storage budget reached: older messages will download when you open them",
                    );
                    break;
                }
                let token = self.tokens.access_token().await?;
                self.fetch_bodies(&token, chunk.to_vec(), true, "Saving offline copies").await?;
            }
            if self.halted() {
                return Ok(());
            }
        }
        self.hist.lock().unwrap_or_else(|e| e.into_inner()).phase = "";
        self.db(|s| s.set_backfill_state(self.account_id, "done"))?;
        let count = self.db(|s| s.message_count(self.account_id))?;
        self.status.log(
            "success",
            format!("Download complete: {count} messages from {} are on this PC", window_label(&window)),
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
            if self.stopped.load(Ordering::Relaxed) || (backfill && self.paused.load(Ordering::Relaxed)) {
                break;
            }
            let before = done;
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
            if backfill {
                self.hist_tick(done - before);
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
            if background {
                self.hist_tick(1);
                if self.halted() {
                    break;
                }
            }
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

    pub fn account_id(&self) -> i64 {
        self.account_id
    }

    pub fn gmail(&self) -> &Gmail {
        &self.gmail
    }

    /// A valid access token (refreshing if needed).
    pub async fn token(&self) -> Result<Secret, SyncError> {
        Ok(self.tokens.access_token().await?)
    }

    /// Run a Gmail call, retrying once with a fresh token after a 401.
    pub async fn with_token<T, F, Fut>(&self, f: F) -> Result<T, SyncError>
    where
        F: Fn(Secret) -> Fut,
        Fut: std::future::Future<Output = Result<T, GmailError>>,
    {
        let token = self.tokens.access_token().await?;
        match f(token).await {
            Err(GmailError::Unauthorized) => {
                self.tokens.invalidate().await;
                let token = self.tokens.access_token().await?;
                Ok(f(token).await?)
            }
            r => Ok(r?),
        }
    }

    /// Full RFC 822 bytes of one message (for attachments, replies).
    pub async fn fetch_raw(&self, message_id: &str) -> Result<Vec<u8>, SyncError> {
        let raw = self.with_token(|t| async move { self.gmail.get_raw(&t, message_id).await }).await?;
        Ok(raw.decode_raw()?)
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
        // Small deltas (the usual push/poll case) are quiet: no bar, no chip
        // text. Large catch-ups (back from offline) or a manual refresh show.
        let visible = total >= LOUD_DELTA || self.status.get().foreground;
        if total > 0 && visible {
            self.status.progress(false, &label, 0, total);
        }
        let (notify_pref, autocopy, hide_locked, multi) = self.db(|s| {
            Ok((
                s.account(self.account_id)?.map(|a| a.notify).unwrap_or_else(|| "all".into()),
                crate::prefs::get(s, "otp_autocopy") == "1",
                crate::prefs::get(s, "otp_lockscreen") == "hide",
                s.accounts()?.len() > 1,
            ))
        })?;
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
            if visible {
                self.status.progress(false, &label, added as u32, total);
            }

            let is_new_inbox = m.label_ids.iter().any(|l| l == "INBOX") && m.label_ids.iter().any(|l| l == "UNREAD");
            let wanted = match notify_pref.as_str() {
                "off" => false,
                "otp" => m.otp_code.is_some(),
                _ => true,
            };
            let fresh = !existed && is_new_inbox && now_ms() - m.internal_date < NOTIFY_MAX_AGE_MS;
            if fresh && autocopy && let Some(code) = &m.otp_code {
                crate::copy_secret(&self.app, code);
                self.status.log("info", "Copied a new verification code to the clipboard");
            }
            if wanted && fresh {
                notify::show_new_mail(
                    &self.app,
                    NewMailToast {
                        account_id: self.account_id,
                        account: if multi { self.tokens.email.clone() } else { String::new() },
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
                        hide_code: hide_locked && m.otp_code.is_some() && notify::session_locked(),
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
