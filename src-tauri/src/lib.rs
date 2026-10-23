//! Pulse Mail core. Everything stateful lives here in Rust; the web UI is a
//! stateless renderer that talks to these modules over IPC.

mod auth;
mod badge;
mod commands;
mod compose;
#[doc(hidden)]
pub mod devtools;
mod files;
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
    /// Attachments kept on this PC (see `files`).
    pub files_dir: PathBuf,
}

impl AppState {
    /// The status cell for an account, created on first use.
    pub fn status_for(&self, app: &AppHandle, account_id: i64, email: &str) -> Arc<sync::status::StatusCell> {
        self.statuses
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(account_id)
            .or_insert_with(|| {
                Arc::new(sync::status::StatusCell::for_account(
                    Some(app.clone()),
                    account_id,
                    email,
                ))
            })
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
        .map(|s| {
            prefs::get_num(
                &s.store.lock().unwrap_or_else(|e| e.into_inner()),
                "clipboard_clear_secs",
            )
        })
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

/// Local log folder (rolling daily files, pruned to a week); None when the
/// platform has no %LOCALAPPDATA% (non-Windows dev/CI: console only).
pub(crate) fn logs_dir() -> Option<PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("com.abdallah.pulsemail").join("logs"))
}

/// Delete rolling log files older than a week (crash logs are diagnostics,
/// not an archive; keeps the folder size-capped without extra state).
fn prune_old_logs(dir: &std::path::Path) {
    const KEEP_SECS: u64 = 7 * 86_400;
    let Some(cutoff) = std::time::SystemTime::now().checked_sub(std::time::Duration::from_secs(KEEP_SECS)) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        if !e.file_name().to_string_lossy().starts_with("pulse-mail.log") {
            continue;
        }
        let old = e
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .is_some_and(|t| t < cutoff);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    let filter = EnvFilter::try_from_env("PULSE_MAIL_LOG")
        .unwrap_or_else(|_| EnvFilter::new("pulse_mail_lib=info,warn,html5ever=error"));
    let console = tracing_subscriber::fmt::layer().with_target(false);
    match logs_dir().filter(|d| std::fs::create_dir_all(d).is_ok()) {
        Some(dir) => {
            prune_old_logs(&dir);
            let appender = tracing_appender::rolling::daily(&dir, "pulse-mail.log");
            let (writer, guard) = tracing_appender::non_blocking(appender);
            // The guard flushes on drop; the app runs until exit, so leak it.
            std::mem::forget(guard);
            let file = tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_ansi(false)
                .with_writer(writer);
            let _ = tracing_subscriber::registry()
                .with(filter)
                .with(console)
                .with(file)
                .try_init();
        }
        None => {
            let _ = tracing_subscriber::registry().with(filter).with(console).try_init();
        }
    }
}

/// True when the wall clock moved far beyond monotonic time: the PC slept or
/// hibernated, so IMAP push may have missed mail. Small drift (< 90 s, e.g.
/// an NTP adjustment) must not trigger a catch-up.
fn clock_jumped(mono_secs: u64, wall_secs: u64) -> bool {
    wall_secs >= mono_secs.saturating_add(90)
}

/// Watch for sleep/resume (ROADMAP X2) and run a quiet catch-up on every
/// account when the machine wakes, instead of waiting out the IMAP backoff.
async fn resume_watch(app: AppHandle) {
    let mut last_mono = std::time::Instant::now();
    let mut last_wall = std::time::SystemTime::now();
    loop {
        tokio::time::sleep(Duration::from_secs(30)).await;
        let mono = last_mono.elapsed().as_secs();
        let wall = last_wall.elapsed().unwrap_or_default().as_secs();
        last_mono = std::time::Instant::now();
        last_wall = std::time::SystemTime::now();
        if !clock_jumped(mono, wall) {
            continue;
        }
        tracing::info!(
            gap_secs = wall.saturating_sub(mono),
            "clock jumped; catching up after sleep"
        );
        if let Some(state) = app.try_state::<AppState>() {
            for (_, c) in state.syncs.lock().await.iter() {
                c.sync_now(false);
            }
        }
    }
}

/// Process-start marker for the cold-start and window-reopen log lines
/// ([PLAN.md] performance table: `startup ready`, `window shown`).
static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

pub(crate) fn started() -> std::time::Instant {
    STARTED.get().copied().unwrap_or_else(std::time::Instant::now)
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
    let started = STARTED.get_or_init(std::time::Instant::now);
    tracing::info!("app starting");
    // One crypto provider for reqwest and IMAP TLS.
    let _ = rustls::crypto::ring::default_provider().install_default();
    // Panics anywhere (sync passes catch theirs; this records every one) go
    // to the rolling log file, then keep the default stderr behavior.
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(panic = %info, "panic captured");
        default_panic(info);
    }));

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
            notify::init(&handle);

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
                files_dir: data_dir.join("attachments"),
            });

            tray::build(&handle)?;
            app.manage(commands::selection::Selections::default());

            let minimized = std::env::args().any(|a| a == "--minimized");
            if !minimized {
                window::show_main(&handle);
            }
            tauri::async_runtime::spawn(commands::resume(handle.clone()));
            tauri::async_runtime::spawn(resume_watch(handle));
            // Cold-start marker: tray built, DB open, engines spawning.
            tracing::info!(ms = started.elapsed().as_millis() as u64, minimized, "startup ready");
            Ok(())
        })
        .on_window_event(window::on_window_event)
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::sign_in,
            commands::sign_out,
            commands::list_labels,
            commands::list_threads,
            commands::category_counts,
            commands::otp_feedback,
            commands::search,
            commands::search_server,
            commands::set_account_settings,
            commands::account_storage,
            commands::get_thread,
            commands::sync_now,
            commands::take_pending_open,
            commands::copy_text,
            commands::open_external,
            commands::open_logs_dir,
            commands::set_destroy_on_close,
            commands::set_autostart,
            commands::load_thread,
            commands::modify_thread,
            commands::selection::create_selection,
            commands::selection::selection_members,
            commands::selection::mark_selection_read,
            commands::get_activity,
            commands::set_notifications,
            commands::account_estimates,
            commands::finish_onboarding,
            commands::set_sync_window,
            commands::pause_history,
            commands::resume_history,
            commands::compose::compose_context,
            commands::compose::save_draft,
            commands::compose::delete_draft,
            commands::compose::send_message,
            commands::compose::cancel_send,
            commands::compose::search_contacts,
            commands::compose::stage_attachment,
            commands::compose::unstage_attachment,
            commands::mail::render_message,
            commands::mail::set_image_sender,
            commands::mail::set_pref,
            commands::mail::save_attachment,
            commands::mail::get_attachment_data,
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

#[cfg(test)]
mod tests {
    use super::clock_jumped;

    #[test]
    fn clock_jump_detects_sleep_not_drift() {
        // Normal ticks: no jump.
        assert!(!clock_jumped(30, 30));
        assert!(!clock_jumped(30, 45));
        // NTP-sized drift stays below the 90 s grace.
        assert!(!clock_jumped(30, 119));
        // A sleep: wall time ran on while timers were paused.
        assert!(clock_jumped(30, 120), "exactly 90 s over counts as a resume");
        assert!(clock_jumped(30, 3_600));
        assert!(clock_jumped(1_800, 7_200), "long poll cycles can also straddle a sleep");
    }
}
