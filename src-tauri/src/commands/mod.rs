//! IPC commands for the UI. Errors are mapped to short, friendly messages;
//! technical details only go to the log (never secrets).

pub mod mail;
pub mod validate;

use crate::AppState;
use crate::auth::client_config::{self, ConfigError};
use crate::auth::oauth::{self, AuthError};
use crate::auth::{TokenManager, token_store};
use crate::store::{LabelRow, MessageRow, ThreadRow};
use crate::sync::gmail::{Gmail, GmailError};
use crate::sync::status::SyncStatus;
use crate::sync::{self, now_ms};
use crate::window::OpenTarget;
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Manager, State};
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
pub struct AppStatus {
    client_configured: bool,
    client_config_path: String,
    account: Option<String>,
    sync: SyncStatus,
    destroy_on_close: bool,
    autostart: bool,
    /// "all" | "otp" | "off"
    notifications: String,
    data_dir: String,
    version: &'static str,
    /// Backend prefs (see prefs.rs), current values.
    prefs: std::collections::BTreeMap<&'static str, String>,
}

#[tauri::command]
pub fn get_status(app: AppHandle, state: State<'_, AppState>) -> CmdResult<AppStatus> {
    let path = client_config::resolve_path(&state.config_dir);
    let configured = client_config::load(&path).is_ok();
    let (account, notifications, prefs) = {
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        (
            store.primary_account()?.map(|a| a.email),
            store.get_setting("notifications")?.unwrap_or_else(|| "all".into()),
            crate::prefs::all(&store),
        )
    };
    Ok(AppStatus {
        client_configured: configured,
        client_config_path: path.display().to_string(),
        account,
        sync: state.status.get(),
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
    })
}

#[tauri::command]
pub async fn sign_in(app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
    let client = client_config::load(&client_config::resolve_path(&state.config_dir))?;
    let pending = oauth::begin_login(&client).await?;
    app.opener().open_url(pending.auth_url.clone(), None::<&str>)?;
    let tokens = pending.finish(&state.http, &client).await?;
    let refresh = tokens
        .refresh_token
        .clone()
        .ok_or_else(|| CmdError::from(AuthError::NoRefreshToken))?;

    let profile = Gmail::new(state.http.clone()).profile(&tokens.access_token).await?;
    let email = profile.email_address;
    token_store::save_refresh_token(&email, &refresh)?;
    let account = state
        .store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .upsert_account(&email, now_ms())?;
    tracing::info!("signed in");

    let manager = TokenManager::new(email.clone(), client, state.http.clone(), refresh);
    start_sync(&app, &state, manager, account.id).await;
    Ok(email)
}

#[tauri::command]
pub async fn sign_out(state: State<'_, AppState>) -> CmdResult<()> {
    if let Some(c) = state.sync.lock().await.take() {
        c.stop();
    }
    let account = state
        .store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .primary_account()?;
    if let Some(a) = account {
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
    }
    state.status.update(|s| *s = SyncStatus::default());
    Ok(())
}

#[tauri::command]
pub fn list_labels(state: State<'_, AppState>) -> CmdResult<Vec<LabelRow>> {
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.primary_account()? {
        Some(a) => Ok(store.list_labels(a.id)?),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub fn list_threads(
    state: State<'_, AppState>,
    label_id: String,
    limit: i64,
    offset: i64,
) -> CmdResult<Vec<ThreadRow>> {
    validate::label_id(&label_id)?;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.primary_account()? {
        Some(a) => Ok(store.list_threads(a.id, &label_id, limit.clamp(1, 5000), offset.max(0))?),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub fn search(state: State<'_, AppState>, query: String, limit: i64) -> CmdResult<Vec<ThreadRow>> {
    validate::text(&query, 500)?;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.primary_account()? {
        Some(a) => Ok(store.search(a.id, &query, limit.clamp(1, 200))?),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub fn get_thread(state: State<'_, AppState>, thread_id: String) -> CmdResult<Vec<MessageRow>> {
    validate::gmail_id(&thread_id)?;
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    match store.primary_account()? {
        Some(a) => Ok(store.get_thread(a.id, &thread_id)?),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub async fn sync_now(state: State<'_, AppState>, manual: Option<bool>) -> CmdResult<()> {
    if let Some(c) = state.sync.lock().await.as_ref() {
        c.sync_now(manual.unwrap_or(true));
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
pub async fn load_thread(state: State<'_, AppState>, thread_id: String) -> CmdResult<Vec<MessageRow>> {
    validate::gmail_id(&thread_id)?;
    let engine = state.sync.lock().await.as_ref().map(|c| c.engine());
    if let Some(engine) = engine {
        // On failure the UI keeps the cached headers + snippet and shows an
        // inline "Couldn't load" state with Retry.
        engine.load_thread_bodies(&thread_id).await?;
    }
    get_thread(state, thread_id)
}

/// Add/remove Gmail labels on a thread: archive (-INBOX), star (+STARRED),
/// read (-UNREAD), unread (+UNREAD). Optimistic; rolled back on failure.
#[tauri::command]
pub async fn modify_thread(
    state: State<'_, AppState>,
    thread_id: String,
    add: Vec<String>,
    remove: Vec<String>,
) -> CmdResult<()> {
    const ALLOWED: &[&str] = &["INBOX", "STARRED", "UNREAD", "IMPORTANT", "SPAM", "TRASH"];
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
    let engine = state
        .sync
        .lock()
        .await
        .as_ref()
        .map(|c| c.engine())
        .ok_or_else(|| CmdError::new("Not signed in."))?;
    engine.modify_thread(&thread_id, &add, &remove).await?;
    Ok(())
}

#[tauri::command]
pub fn get_activity(state: State<'_, AppState>) -> Vec<crate::sync::status::Activity> {
    state.status.activity()
}

#[tauri::command]
pub fn set_notifications(state: State<'_, AppState>, mode: String) -> CmdResult<()> {
    if !matches!(mode.as_str(), "all" | "otp" | "off") {
        return Err(CmdError::new("Unknown notification setting."));
    }
    state
        .store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .set_setting("notifications", &mode)?;
    Ok(())
}

/// (Re)start the sync engine for an account.
pub async fn start_sync(app: &AppHandle, state: &AppState, tokens: TokenManager, account_id: i64) {
    let mut slot = state.sync.lock().await;
    if let Some(old) = slot.take() {
        old.stop();
    }
    *slot = Some(sync::spawn(
        app.clone(),
        state.store.clone(),
        Arc::new(tokens),
        state.http.clone(),
        account_id,
        state.status.clone(),
    ));
}

/// On launch: resume syncing if we have an account, a client file and a
/// refresh token in Credential Manager. Silent if anything is missing.
pub async fn resume(app: AppHandle) {
    let state = app.state::<AppState>();
    let Ok(Some(account)) = state.store.lock().unwrap_or_else(|e| e.into_inner()).primary_account() else {
        return;
    };
    let Ok(client) = client_config::load(&client_config::resolve_path(&state.config_dir)) else {
        return;
    };
    let refresh = match token_store::load_refresh_token(&account.email) {
        Ok(Some(t)) => t,
        Ok(None) => {
            state.status.update(|s| s.error = Some("Please sign in again.".into()));
            return;
        }
        Err(e) => {
            tracing::warn!(error = %e, "credential manager read failed");
            return;
        }
    };
    let manager = TokenManager::new(account.email.clone(), client, state.http.clone(), refresh);
    start_sync(&app, &state, manager, account.id).await;
}
