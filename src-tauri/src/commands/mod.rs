//! IPC commands for the UI. Errors are mapped to short, friendly messages;
//! technical details only go to the log (never secrets).

pub mod compose;
pub mod mail;
pub mod selection;
pub mod validate;

use crate::AppState;
use crate::auth::client_config::{self, ConfigError};
use crate::auth::oauth::{self, AuthError};
use crate::auth::{TokenManager, token_store};
use crate::store::{Account, LabelRow, MessageRow, SYNC_WINDOWS, ThreadRow};
use crate::sync::gmail::{Gmail, GmailError};
use crate::sync::status::{Activity, SyncStatus};
use crate::sync::{self, now_ms};
use crate::window::OpenTarget;
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_opener::OpenerExt as _;

#[derive(Debug, Serialize)]
pub struct CmdError {
    message: String,
}

impl CmdError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

macro_rules! friendly {
    ($ty:ty, $e:ident => $msg:expr) => {
        impl From<$ty> for CmdError {
            fn from($e: $ty) -> Self {
                tracing::warn!(error = %$e, "command failed");
                CmdError::new($msg)
            }
        }
    };
}

friendly!(rusqlite::Error, _e => "Something went wrong with local storage.");
friendly!(keyring::Error, _e => "Couldn't access Windows Credential Manager.");
friendly!(tauri::Error, _e => "Something went wrong. Please try again.");
friendly!(tauri_plugin_opener::Error, _e => "Couldn't open your browser.");
friendly!(GmailError, _e => "Couldn't reach Gmail. Check your connection and try again.");
friendly!(sync::SyncError, e => e.friendly());
friendly!(ConfigError, e => match e {
    ConfigError::Missing(_) => "Pulse Mail isn't connected to a Google OAuth client yet. See the setup steps.",
    ConfigError::Invalid(_) => "The Google OAuth client file isn't valid. Download it again from Google Cloud.",
});
friendly!(AuthError, e => match e {
    AuthError::Cancelled => "Sign-in was cancelled or took too long.",
    AuthError::Denied(_) => "Google sign-in was declined.",
    AuthError::InvalidGrant => "Your Google sign-in expired. Please sign in again.",
    AuthError::NoRefreshToken => "Google didn't grant offline access. Please try signing in again.",
    AuthError::Network(_) => "You're offline. Check your connection and try again.",
    _ => "Sign-in didn't work. Please try again.",
});

pub(crate) type CmdResult<T> = Result<T, CmdError>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    #[serde(flatten)]
    account: Account,
    sync: SyncStatus,
    /// Unread INBOX threads.
    unread: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatus {
    client_configured: bool,
    client_config_path: String,
    /// Email of the default (oldest) account, if any.
    account: Option<String>,
    accounts: Vec<AccountInfo>,
    destroy_on_close: bool,
    autostart: bool,
    /// Default notification mode for new accounts: "all" | "primary" | "otp" | "off"
    notifications: String,
    data_dir: String,
    version: &'static str,
    /// Backend prefs (see prefs.rs), current values.
    prefs: std::collections::BTreeMap<&'static str, String>,
    /// Database size in bytes (for the storage budget).
    db_bytes: i64,
}

fn account_infos(state: &AppState, store: &crate::store::Store) -> rusqlite::Result<Vec<AccountInfo>> {
    let unread: std::collections::HashMap<i64, i64> = store.inbox_unread()?.into_iter().collect();
    Ok(store
        .accounts()?
        .into_iter()
        .map(|a| AccountInfo {
            sync: state.status_snapshot(a.id).unwrap_or_else(|| SyncStatus {
                account_id: a.id,
                email: a.email.clone(),
                ..Default::default()
            }),
            unread: unread.get(&a.id).copied().unwrap_or(0),
            account: a,
        })
        .collect())
}

#[tauri::command]
pub fn get_status(app: AppHandle, state: State<'_, AppState>) -> CmdResult<AppStatus> {
    let path = client_config::resolve_path(&state.config_dir);
    let configured = client_config::load(&path).is_ok();
    let (accounts, notifications, prefs, db_bytes) = {
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        (
            account_infos(&state, &store)?,
            store.get_setting("notifications")?.unwrap_or_else(|| "all".into()),
            crate::prefs::all(&store),
            store.db_size().unwrap_or(0),
        )
    };
    Ok(AppStatus {
        client_configured: configured,
        client_config_path: path.display().to_string(),
        account: accounts.first().map(|a| a.account.email.clone()),
        accounts,
        destroy_on_close: state.destroy_on_close.load(Ordering::Relaxed),
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        notifications,
        data_dir: app
            .path()
            .app_data_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default(),
        version: env!("CARGO_PKG_VERSION"),
        prefs,
        db_bytes,
    })
}

/// The named account, or the default one when `account_id` is None.
pub(crate) fn resolve_account(state: &AppState, account_id: Option<i64>) -> CmdResult<Account> {
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    let found = match account_id {
        Some(id) => store.account(id)?,
        None => store.primary_account()?,
    };
    found.ok_or_else(|| CmdError::new("Not signed in."))
}

/// The running engine of an account (default account when None).
pub(crate) async fn engine_for(state: &AppState, account_id: Option<i64>) -> CmdResult<Arc<sync::Engine>> {
    let id = resolve_account(state, account_id)?.id;
    state
        .syncs
        .lock()
        .await
        .get(&id)
        .map(|c| c.engine())
        .ok_or_else(|| CmdError::new("That account isn't connected right now."))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedIn {
    account_id: i64,
    email: String,
    messages_total: i64,
    threads_total: i64,
    /// The account still needs onboarding choices (download range, alerts).
    needs_setup: bool,
}

/// Sign in with Google (adds an account; signing in to an existing one
/// refreshes its credentials). Sync starts right away so the inbox fills
/// while onboarding continues; the history download waits for
/// `finish_onboarding`.
#[tauri::command]
pub async fn sign_in(app: AppHandle, state: State<'_, AppState>, account_id: Option<i64>) -> CmdResult<SignedIn> {
    let expected = account_id.map(|id| resolve_account(&state, Some(id))).transpose()?;
    let client = client_config::load(&client_config::resolve_path(&state.config_dir))?;
    let pending = oauth::begin_login(&client, expected.as_ref().map(|a| a.email.as_str())).await?;
    app.opener().open_url(pending.auth_url.clone(), None::<&str>)?;
    let tokens = pending.finish(&state.http, &client).await?;
    let refresh = tokens
        .refresh_token
        .clone()
        .ok_or_else(|| CmdError::from(AuthError::NoRefreshToken))?;

    let profile = Gmail::new(state.http.clone()).profile(&tokens.access_token).await?;
    let email = profile.email_address;
    if expected.as_ref().is_some_and(|a| !a.email.eq_ignore_ascii_case(&email)) {
        return Err(CmdError::new("Choose the same Google account to reconnect."));
    }
    token_store::save_refresh_token(&email, &refresh)?;
    let account = {
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        let a = store.upsert_account(&email, now_ms())?;
        store.set_messages_total(a.id, profile.messages_total)?;
        a
    };
    tracing::info!(account = account.id, "signed in");

    let manager = TokenManager::new(email.clone(), client, state.http.clone(), refresh);
    start_sync(&app, &state, manager, &account).await;
    // Optional identity work must not hold up the inbox or sign-in result.
    let (profile_app, profile_email, profile_id) = (app.clone(), email.clone(), account.id);
    tauri::async_runtime::spawn(async move {
        let Ok(profile) = crate::auth::profile::fetch(&tokens.access_token).await else {
            tracing::debug!("Google profile unavailable; keeping cached account details");
            return;
        };
        let state = profile_app.state::<AppState>();
        let updated = {
            let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
            // Removal/re-addition while the request was in flight may reuse an id.
            match store.account(profile_id) {
                Ok(Some(a)) if a.email == profile_email => store
                    .set_google_profile(profile_id, &profile.name, profile.avatar_data.as_deref())
                    .unwrap_or(false),
                _ => false,
            }
        };
        if updated {
            let _ = profile_app.emit("accounts-changed", ());
        }
    });
    Ok(SignedIn {
        account_id: account.id,
        email,
        messages_total: profile.messages_total,
        threads_total: profile.threads_total,
        needs_setup: account.backfill_state == "setup",
    })
}

/// Remove one account from this PC (default account when None): stops its
/// sync, revokes and deletes its sign-in, and deletes its cached mail.
#[tauri::command]
pub async fn sign_out(state: State<'_, AppState>, account_id: Option<i64>) -> CmdResult<()> {
    let a = resolve_account(&state, account_id)?;
    if let Some(c) = state.syncs.lock().await.remove(&a.id) {
        c.stop();
    }
    if let Ok(Some(rt)) = token_store::load_refresh_token(&a.email) {
        // Best effort: also revoke at Google.
        let _ = oauth::revoke(&state.http, &rt).await;
    }
    token_store::delete_refresh_token(&a.email)?;
    state
        .store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .delete_account(a.id)?;
    state.statuses.lock().unwrap_or_else(|e| e.into_inner()).remove(&a.id);
    crate::files::remove_account(&state.files_dir, a.id);
    Ok(())
}

/// Change per-account settings (Settings > account page).
#[tauri::command]
pub async fn set_account_settings(
    state: State<'_, AppState>,
    account_id: i64,
    settings: crate::store::AccountSettings,
) -> CmdResult<()> {
    let a = resolve_account(&state, Some(account_id))?;
    if let Some(n) = &settings.display_name {
        validate::text(n, 200)?;
        if n.chars().count() > 80 || n.chars().any(char::is_control) {
            return Err(CmdError::new("That name is too long."));
        }
    }
    if let Some(c) = &settings.color
        && !(c.is_empty() || (c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit())))
    {
        return Err(CmdError::new("Unknown color."));
    }
    if let Some(sig) = &settings.signature {
        validate::text(sig, 4000)?;
    }
    if let Some(m) = &settings.attach_mode
        && !crate::store::ATTACH_MODES.contains(&m.as_str())
    {
        return Err(CmdError::new("Unknown attachment setting."));
    }
    if let Some(mb) = settings.attach_max_mb
        && !(1..=50).contains(&mb)
    {
        return Err(CmdError::new("Choose a limit between 1 and 50 MB."));
    }
    let policy_changed = settings.attach_mode.as_deref().is_some_and(|m| m != a.attach_mode)
        || settings.attach_max_mb.is_some_and(|mb| mb != a.attach_max_mb);
    {
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        store.set_account_settings(a.id, &settings)?;
        // A more generous policy: fetch the attachments it now allows.
        if policy_changed && a.backfill_state == "done" {
            store.set_backfill_phase(a.id, "files")?;
            store.set_backfill_state(a.id, "pending")?;
        }
    }
    if policy_changed && a.backfill_state == "done" {
        engine_for(&state, Some(a.id)).await?.resume_history()?;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountStorage {
    messages: i64,
    /// Approximate bytes of this account's mail in the database.
    db_bytes: i64,
    /// Attachments saved on this PC.
    files_bytes: i64,
}

#[tauri::command]
pub fn account_storage(state: State<'_, AppState>, account_id: i64) -> CmdResult<AccountStorage> {
    let a = resolve_account(&state, Some(account_id))?;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    Ok(AccountStorage {
        messages: store.message_count(a.id)?,
        db_bytes: store.account_db_bytes(a.id)?,
        files_bytes: store.stored_attachment_bytes(Some(a.id))?,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowEstimate {
    window: &'static str,
    /// Gmail's estimate of matching messages (rough for large mailboxes).
    messages: i64,
    /// Estimated disk use of the full messages (without attachments).
    bytes: i64,
}

/// Average on-disk cost per message: metadata row + search index, and a
/// cached body (text + HTML source; inline images excluded).
const META_BYTES: i64 = 3 * 1024;
const BODY_BYTES: i64 = 45 * 1024;

/// Per-range size estimates for the onboarding "what to download" step.
#[tauri::command]
pub async fn account_estimates(state: State<'_, AppState>, account_id: i64) -> CmdResult<Vec<WindowEstimate>> {
    let engine = engine_for(&state, Some(account_id)).await?;
    let e = engine.as_ref();
    let count = |q: &'static str| async move {
        e.with_token(|t| async move { e.gmail().list_page(&t, q, None, 1).await })
            .await
            .map(|p| p.estimate)
    };
    let (d30, d90, y1, profile) = tokio::try_join!(
        count("newer_than:30d"),
        count("newer_than:90d"),
        count("newer_than:1y"),
        e.with_token(|t| async move { e.gmail().profile(&t).await }),
    )?;
    let all = profile.messages_total.max(y1);
    // The range is downloaded in full (text and HTML), attachments per the
    // account's attachment setting (not included here).
    let est = |window: &'static str, n: i64| WindowEstimate {
        window,
        messages: n,
        bytes: n * (META_BYTES + BODY_BYTES),
    };
    Ok(vec![
        est("30d", d30),
        est("90d", d90.max(d30)),
        est("1y", y1.max(d90)),
        est("all", all),
    ])
}

fn check_window(window: &str) -> CmdResult<()> {
    if SYNC_WINDOWS.contains(&window) {
        Ok(())
    } else {
        Err(CmdError::new("Unknown download range."))
    }
}

fn check_notify(mode: &str) -> CmdResult<()> {
    if matches!(mode, "all" | "primary" | "otp" | "off") {
        Ok(())
    } else {
        Err(CmdError::new("Unknown notification setting."))
    }
}

/// Last onboarding step: save the choices and start the history download.
#[tauri::command]
pub async fn finish_onboarding(
    state: State<'_, AppState>,
    account_id: i64,
    window: String,
    notify: String,
) -> CmdResult<()> {
    check_window(&window)?;
    check_notify(&notify)?;
    {
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        store.set_account_window(account_id, &window)?;
        store.set_account_notify(account_id, &notify)?;
        store.set_backfill_state(account_id, "pending")?;
    }
    engine_for(&state, Some(account_id)).await?.resume_history()?;
    Ok(())
}

/// Change how much history an account keeps (Settings > Accounts).
#[tauri::command]
pub async fn set_sync_window(state: State<'_, AppState>, account_id: i64, window: String) -> CmdResult<()> {
    check_window(&window)?;
    let a = resolve_account(&state, Some(account_id))?;
    state
        .store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .set_account_window(a.id, &window)?;
    if a.backfill_state != "setup" {
        engine_for(&state, Some(a.id)).await?.resume_history()?;
    }
    Ok(())
}

#[tauri::command]
pub async fn pause_history(state: State<'_, AppState>, account_id: i64) -> CmdResult<()> {
    engine_for(&state, Some(account_id)).await?.pause_history()?;
    Ok(())
}

#[tauri::command]
pub async fn resume_history(state: State<'_, AppState>, account_id: i64) -> CmdResult<()> {
    engine_for(&state, Some(account_id)).await?.resume_history()?;
    Ok(())
}

/// Bump when the OTP detector changes enough to re-score cached mail.
const OTP_VERSION: &str = "5";

/// Re-run OTP detection over the cache once per detector version, in small
/// batches inside transactions so the store lock is never held for long.
fn rescan_otp_if_needed(app: &AppHandle) {
    let state = app.state::<AppState>();
    let lock = || state.store.lock().unwrap_or_else(|e| e.into_inner());
    if lock().get_setting("otp_version").ok().flatten().as_deref() == Some(OTP_VERSION) {
        return;
    }
    let started = std::time::Instant::now();
    let (mut after, mut scanned, mut found) = (0i64, 0usize, 0usize);
    loop {
        let Ok(batch) = lock().otp_rescan_batch(after, 200) else {
            return;
        };
        if batch.is_empty() {
            break;
        }
        let mut updates = Vec::new();
        for r in &batch {
            after = r.rowid;
            scanned += 1;
            let d = crate::sync::mime::safe_detect(&crate::otp::Input {
                subject: &r.subject,
                text: &r.text,
                html: if r.body_loaded { r.html.as_deref() } else { None },
                from: &r.from,
                promotional: r.promotional,
            });
            let (code, link, score) = match &d {
                Some(d) if d.kind == crate::otp::Kind::Code => (Some(d.value.clone()), None, d.confidence),
                Some(d) => (None, Some(d.value.clone()), d.confidence),
                None => (None, None, 0),
            };
            if d.is_some() {
                found += 1;
            }
            if code != r.current_code || link != r.current_link || score != r.current_score {
                updates.push(crate::store::OtpRescanUpdate {
                    account_id: r.account_id,
                    id: r.id.clone(),
                    from: r.from.clone(),
                    code,
                    link,
                    score,
                });
            }
        }
        if !updates.is_empty() {
            let mut store = lock();
            if let Err(e) = store.set_otp_batch(&updates) {
                tracing::warn!(error = %e, "otp rescan write failed");
                return;
            }
        }
        // Yield execution between batches so UI queries acquire the lock without starvation.
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    let _ = lock().set_setting("otp_version", OTP_VERSION);
    tracing::info!(
        scanned,
        found,
        ms = started.elapsed().as_millis() as u64,
        "otp rescan done"
    );
    let _ = app.emit("mail-updated", ());
}

/// Labels of one account; with `account_id` None and several accounts, the
/// unified view: system labels with counts summed across accounts.
#[tauri::command]
pub fn list_labels(state: State<'_, AppState>, account_id: Option<i64>) -> CmdResult<Vec<LabelRow>> {
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(id) = account_id {
        return Ok(store.list_labels(id)?);
    }
    match store.accounts()?.as_slice() {
        [] => Ok(Vec::new()),
        [only] => Ok(store.list_labels(only.id)?),
        many => {
            let mut merged: Vec<LabelRow> = Vec::new();
            for a in many {
                for l in store.list_labels(a.id)? {
                    match merged.iter_mut().find(|m| m.id == l.id) {
                        Some(m) => {
                            m.unread += l.unread;
                            m.total += l.total;
                        }
                        None => merged.push(l),
                    }
                }
            }
            Ok(merged)
        }
    }
}

/// Threads in a label. `account_id` None = every account (unified inbox).
#[tauri::command]
pub fn list_threads(
    state: State<'_, AppState>,
    label_id: String,
    limit: i64,
    offset: i64,
    account_id: Option<i64>,
    category: Option<String>,
) -> CmdResult<Vec<ThreadRow>> {
    validate::label_id(&label_id)?;
    if let Some(c) = &category
        && !crate::store::CATEGORIES.contains(&c.as_str())
    {
        return Err(CmdError::new("Unknown inbox tab."));
    }
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    Ok(store.list_threads_in(
        account_id,
        &label_id,
        category.as_deref(),
        limit.clamp(1, 5000),
        offset.max(0),
    )?)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCount {
    pub id: String,
    pub unread: i64,
    pub total: i64,
}

/// Gmail inbox tabs (Primary, Promotions, Social, Updates, Forums) with
/// unread and total thread counts, for one account or all.
#[tauri::command]
pub fn category_counts(state: State<'_, AppState>, account_id: Option<i64>) -> CmdResult<Vec<CategoryCount>> {
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    let counts = store.category_counts(account_id)?;
    Ok(crate::store::CATEGORIES
        .iter()
        .map(|id| {
            let (unread, total) = counts
                .iter()
                .find(|(c, _, _)| c == id)
                .map(|(_, u, t)| (*u, *t))
                .unwrap_or_default();
            CategoryCount {
                id: (*id).to_string(),
                unread,
                total,
            }
        })
        .collect())
}

/// "Not a code" (useful = false, also clears that message's code) or a
/// copied code (useful = true): adjusts future detection for the sender.
#[tauri::command]
pub fn otp_feedback(
    state: State<'_, AppState>,
    account_id: i64,
    sender: String,
    useful: bool,
    message_id: Option<String>,
) -> CmdResult<()> {
    validate::text(&sender, 320)?;
    if let Some(id) = &message_id {
        validate::gmail_id(id)?;
    }
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    store.otp_feedback(account_id, &sender, useful, message_id.as_deref())?;
    Ok(())
}

#[tauri::command]
pub fn search(
    state: State<'_, AppState>,
    query: String,
    limit: i64,
    account_id: Option<i64>,
    offset: Option<i64>,
) -> CmdResult<Vec<ThreadRow>> {
    validate::text(&query, 500)?;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    Ok(store.search_page(account_id, &query, limit.clamp(1, 200), offset.unwrap_or(0).max(0))?)
}

/// Search Gmail itself (all mail, every operator) for one account or all,
/// merged with nothing: the UI merges these rows with the local results.
#[tauri::command]
pub async fn search_server(
    state: State<'_, AppState>,
    query: String,
    limit: Option<u32>,
    account_id: Option<i64>,
) -> CmdResult<Vec<ThreadRow>> {
    validate::text(&query, 500)?;
    if query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let max = limit.unwrap_or(50).clamp(1, 100);
    let engines: Vec<Arc<sync::Engine>> = {
        let syncs = state.syncs.lock().await;
        syncs
            .iter()
            .filter(|(id, _)| account_id.is_none_or(|a| a == **id))
            .map(|(_, c)| c.engine())
            .collect()
    };
    if engines.is_empty() {
        return Err(CmdError::new("That account isn't connected right now."));
    }
    let results = futures::future::join_all(engines.iter().map(|e| {
        let q = query.clone();
        async move { (e.account_id(), e.server_search(&q, max).await) }
    }))
    .await;
    let mut rows = Vec::new();
    let mut failed = 0;
    for (id, r) in results {
        match r {
            Ok(ids) => {
                let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
                rows.extend(store.threads_for_messages(id, &ids)?);
            }
            Err(e) => {
                tracing::warn!(error = %e, "server search failed");
                failed += 1;
            }
        }
    }
    if failed == engines.len() {
        return Err(CmdError::new("Couldn't search Gmail. Showing mail saved on this PC."));
    }
    rows.sort_by_key(|r| std::cmp::Reverse(r.internal_date));
    Ok(rows)
}

#[tauri::command]
pub fn get_thread(
    state: State<'_, AppState>,
    thread_id: String,
    account_id: Option<i64>,
) -> CmdResult<Vec<MessageRow>> {
    validate::gmail_id(&thread_id)?;
    let id = resolve_account(&state, account_id)?.id;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    Ok(store.get_thread(id, &thread_id)?)
}

/// Check for mail now. `manual` (default true) = the user asked, so the UI
/// shows it. `account_id` None = every account.
#[tauri::command]
pub async fn sync_now(state: State<'_, AppState>, manual: Option<bool>, account_id: Option<i64>) -> CmdResult<()> {
    let syncs = state.syncs.lock().await;
    for (id, c) in syncs.iter() {
        if account_id.is_none_or(|a| a == *id) {
            c.sync_now(manual.unwrap_or(true));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn take_pending_open(state: State<'_, AppState>) -> Option<OpenTarget> {
    state.pending_open.lock().unwrap().take()
}

#[tauri::command]
pub fn copy_text(app: AppHandle, text: String, secret: Option<bool>) -> CmdResult<()> {
    validate::text(&text, 10_000)?;
    if secret.unwrap_or(false) {
        crate::copy_secret(&app, &text);
    } else {
        crate::copy_to_clipboard(&app, &text);
    }
    Ok(())
}

/// Open a link from an email in the default browser. Only web and mail links.
#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> CmdResult<()> {
    let parsed = url::Url::parse(&url).map_err(|_| CmdError::new("That link can't be opened."))?;
    if !matches!(parsed.scheme(), "https" | "http" | "mailto") {
        return Err(CmdError::new("That link can't be opened."));
    }
    app.opener().open_url(parsed.as_str(), None::<&str>)?;
    Ok(())
}

#[tauri::command]
pub fn set_destroy_on_close(state: State<'_, AppState>, enabled: bool) -> CmdResult<()> {
    state
        .store
        .lock()
        .unwrap()
        .set_setting("destroy_on_close", if enabled { "1" } else { "0" })?;
    state.destroy_on_close.store(enabled, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> CmdResult<bool> {
    let al = app.autolaunch();
    let r = if enabled { al.enable() } else { al.disable() };
    r.map_err(|e| {
        tracing::warn!(error = %e, "autostart change failed");
        CmdError::new("Couldn't change the start-with-Windows setting.")
    })?;
    Ok(al.is_enabled().unwrap_or(enabled))
}

/// Thread for the reader. Downloads any missing bodies first (metadata-only
/// rows from the fast first sync), then returns the cached thread.
#[tauri::command]
pub async fn load_thread(
    state: State<'_, AppState>,
    thread_id: String,
    account_id: Option<i64>,
) -> CmdResult<Vec<MessageRow>> {
    validate::gmail_id(&thread_id)?;
    if let Ok(engine) = engine_for(&state, account_id).await {
        // On failure the UI keeps the cached headers + snippet and shows an
        // inline "Couldn't load" state with Retry.
        engine.load_thread_bodies(&thread_id).await?;
    }
    get_thread(state, thread_id, account_id)
}

/// Add/remove Gmail labels on a thread: archive (-INBOX), star (+STARRED),
/// read (-UNREAD), unread (+UNREAD). Optimistic; rolled back on failure.
#[tauri::command]
pub async fn modify_thread(
    state: State<'_, AppState>,
    thread_id: String,
    add: Vec<String>,
    remove: Vec<String>,
    account_id: Option<i64>,
) -> CmdResult<()> {
    // CATEGORY_* moves a thread between Gmail's inbox tabs (allowed by the
    // existing https://mail.google.com/ scope).
    const ALLOWED: &[&str] = &[
        "INBOX",
        "STARRED",
        "UNREAD",
        "IMPORTANT",
        "SPAM",
        "TRASH",
        "CATEGORY_PERSONAL",
        "CATEGORY_SOCIAL",
        "CATEGORY_PROMOTIONS",
        "CATEGORY_UPDATES",
        "CATEGORY_FORUMS",
    ];
    validate::gmail_id(&thread_id)?;
    if add.len() + remove.len() > 10 {
        return Err(CmdError::new("That action isn't supported."));
    }
    for l in add.iter().chain(&remove) {
        validate::label_id(l)?;
    }
    if add
        .iter()
        .chain(&remove)
        .any(|l| !ALLOWED.contains(&l.as_str()) && !l.starts_with("Label_"))
    {
        return Err(CmdError::new("That action isn't supported."));
    }
    let engine = engine_for(&state, account_id).await?;
    engine.modify_thread(&thread_id, &add, &remove).await?;
    Ok(())
}

/// Recent activity of every account, oldest first.
#[tauri::command]
pub fn get_activity(state: State<'_, AppState>) -> Vec<Activity> {
    let mut all: Vec<Activity> = state
        .statuses
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .values()
        .flat_map(|c| c.activity())
        .collect();
    all.sort_by_key(|a| a.at);
    let skip = all.len().saturating_sub(60);
    all.split_off(skip)
}

/// Notification mode for one account, or (None) the default for new
/// accounts and every existing account.
#[tauri::command]
pub fn set_notifications(state: State<'_, AppState>, mode: String, account_id: Option<i64>) -> CmdResult<()> {
    check_notify(&mode)?;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match account_id {
        Some(id) => store.set_account_notify(id, &mode)?,
        None => {
            store.set_setting("notifications", &mode)?;
            for a in store.accounts()? {
                store.set_account_notify(a.id, &mode)?;
            }
        }
    }
    Ok(())
}

/// (Re)start the sync engine for one account (other accounts keep running).
pub async fn start_sync(app: &AppHandle, state: &AppState, tokens: TokenManager, account: &Account) {
    let status = state.status_for(app, account.id, &account.email);
    status.update(|s| s.error = None);
    let mut syncs = state.syncs.lock().await;
    if let Some(old) = syncs.remove(&account.id) {
        old.stop();
    }
    syncs.insert(
        account.id,
        sync::spawn(
            app.clone(),
            state.store.clone(),
            Arc::new(tokens),
            state.http.clone(),
            account.id,
            status,
        ),
    );
}

/// On launch: resume syncing every account that has a refresh token in
/// Credential Manager. Accounts without one show "Please sign in again".
pub async fn resume(app: AppHandle) {
    let state = app.state::<AppState>();
    let rescan_app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Let initial window paint and initial queries finish before starting background rescan.
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        tauri::async_runtime::spawn_blocking(move || rescan_otp_if_needed(&rescan_app));
    });
    let Ok(accounts) = state.store.lock().unwrap_or_else(|e| e.into_inner()).accounts() else {
        return;
    };
    let Ok(client) = client_config::load(&client_config::resolve_path(&state.config_dir)) else {
        return;
    };
    for account in accounts {
        match token_store::load_refresh_token(&account.email) {
            Ok(Some(refresh)) => {
                let manager = TokenManager::new(account.email.clone(), client.clone(), state.http.clone(), refresh);
                start_sync(&app, &state, manager, &account).await;
            }
            Ok(None) => {
                state
                    .status_for(&app, account.id, &account.email)
                    .update(|s| s.error = Some("Please sign in again.".into()));
            }
            Err(e) => tracing::warn!(error = %e, "credential manager read failed"),
        }
    }
}
