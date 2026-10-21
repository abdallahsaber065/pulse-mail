//! Pulse Mail core. Everything stateful lives here in Rust; the web UI is a
//! stateless renderer that talks to these modules over IPC.

mod auth;
mod badge;
mod commands;
#[doc(hidden)]
pub mod devtools;
mod notify;
mod otp;
mod prefs;
mod render;
mod secure_clipboard;
mod security;
mod store;
mod sync;
mod tray;
mod window;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Manager, RunEvent};

pub struct AppState {
    pub store: Arc<Mutex<store::Store>>,
    pub http: reqwest::Client,
    pub config_dir: PathBuf,
    /// One running sync engine per signed-in account.
    pub syncs: tokio::sync::Mutex<BTreeMap<i64, sync::SyncController>>,
    /// Status cell per account (also for accounts that can't sync right now,
    /// e.g. a missing sign-in, so the UI can say so).
    pub statuses: Mutex<BTreeMap<i64, Arc<sync::status::StatusCell>>>,
    pub pending_open: Mutex<Option<window::OpenTarget>>,
    pub destroy_on_close: AtomicBool,
    pub images: render::proxy::ImageProxy,
}

impl AppState {
    /// The status cell for an account, created on first use.
    pub fn status_for(&self, app: &AppHandle, account_id: i64, email: &str) -> Arc<sync::status::StatusCell> {
        self.statuses
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(account_id)
            .or_insert_with(|| Arc::new(sync::status::StatusCell::for_account(Some(app.clone()), account_id, email)))
            .clone()
    }

    pub fn status_snapshot(&self, account_id: i64) -> Option<sync::status::SyncStatus> {
        self.statuses
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&account_id)
            .map(|c| c.get())
    }
}

pub(crate) fn copy_to_clipboard(app: &AppHandle, text: &str) {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    if let Err(e) = app.clipboard().write_text(text.to_string()) {
        tracing::warn!(error = %e, "clipboard write failed");
    }
}

/// Copy a secret (verification code) and clear it again after the
/// `clipboard_clear_secs` pref, unless something else was copied meanwhile.
pub(crate) fn copy_secret(app: &AppHandle, text: &str) {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    // Kept out of Windows clipboard history and cloud clipboard when possible.
    if !secure_clipboard::write_secret(text) {
        copy_to_clipboard(app, text);
    }
    let secs = app
        .try_state::<AppState>()
        .map(|s| prefs::get_num(&s.store.lock().unwrap_or_else(|e| e.into_inner()), "clipboard_clear_secs"))
        .unwrap_or(60);
    if secs == 0 {
        return;
    }
    let (app, text) = (app.clone(), text.to_string());
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(secs)).await;
        if app.clipboard().read_text().is_ok_and(|cur| cur == text) {
            if !secure_clipboard::clear() {
                let _ = app.clipboard().write_text(String::new());
            }
            tracing::info!("cleared a copied code from the clipboard");
        }
    });
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter =
        EnvFilter::try_from_env("PULSE_MAIL_LOG").unwrap_or_else(|_| EnvFilter::new("pulse_mail_lib=info,warn,html5ever=error"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(concat!("PulseMail/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(60))
        .pool_idle_timeout(Duration::from_secs(90))
        .build()
        .expect("http client")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();
    // One crypto provider for reqwest and IMAP TLS.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let app = tauri::Builder::default()
        // Must be first: a second launch just focuses the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show_main(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            let handle = app.handle().clone();
            let config_dir = app.path().app_config_dir()?;
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            std::fs::create_dir_all(&data_dir)?;

            let store = store::Store::open(&data_dir.join("pulse-mail.db"))?;
            let destroy_on_close = store.get_setting("destroy_on_close")?.as_deref() == Some("1");

            app.manage(AppState {
                store: Arc::new(Mutex::new(store)),
                http: http_client(),
                config_dir,
                syncs: tokio::sync::Mutex::new(BTreeMap::new()),
                statuses: Mutex::new(BTreeMap::new()),
                pending_open: Mutex::new(None),
                destroy_on_close: AtomicBool::new(destroy_on_close),
                images: render::proxy::ImageProxy::new(),
            });

            tray::build(&handle)?;

            let minimized = std::env::args().any(|a| a == "--minimized");
            if !minimized {
                window::show_main(&handle);
            }
            tauri::async_runtime::spawn(commands::resume(handle));
            Ok(())
        })
        .on_window_event(window::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::sign_in,
            commands::sign_out,
            commands::list_labels,
            commands::list_threads,
            commands::search,
            commands::get_thread,
            commands::sync_now,
            commands::take_pending_open,
            commands::copy_text,
            commands::open_external,
            commands::set_destroy_on_close,
            commands::set_autostart,
            commands::load_thread,
            commands::modify_thread,
            commands::get_activity,
            commands::set_notifications,
            commands::account_estimates,
            commands::finish_onboarding,
            commands::set_sync_window,
            commands::pause_history,
            commands::resume_history,
            commands::mail::render_message,
            commands::mail::set_image_sender,
            commands::mail::set_pref,
            commands::mail::save_attachment,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Pulse Mail");

    app.run(|_app, event| {
        // Closing the last window must not quit: the tray + sync keep running.
        // Tray "Quit" calls app.exit(0), which carries an exit code.
        if let RunEvent::ExitRequested { code: None, api, .. } = event {
            api.prevent_exit();
        }
    });
}
