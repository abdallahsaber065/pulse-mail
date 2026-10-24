//! Main window lifecycle: start hidden in the tray, hide on close (fast
//! reopen) or destroy on close (smallest memory), recreate on demand.

use crate::AppState;
use serde::Serialize;
use std::sync::atomic::Ordering;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow, WebviewWindowBuilder, Window, WindowEvent};

pub const MAIN: &str = "main";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenTarget {
    pub account_id: i64,
    pub thread_id: String,
    pub message_id: Option<String>,
}

pub fn show_main(app: &AppHandle) {
    let fresh = app.get_webview_window(MAIN).is_none();
    let window = match app.get_webview_window(MAIN) {
        Some(w) => w,
        None => match create_main(app) {
            Ok(w) => {
                // A recreated window has no taskbar overlay yet.
                crate::tray::schedule_badge(app);
                w
            }
            Err(e) => {
                tracing::error!(error = %e, "could not create main window");
                return;
            }
        },
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
    // Open/reopen marker for the PLAN.md window-latency targets.
    tracing::info!(
        ms = crate::started().elapsed().as_millis() as u64,
        fresh,
        "window shown"
    );
}

fn create_main(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let conf = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == MAIN)
        .cloned()
        .expect("main window is declared in tauri.conf.json");
    WebviewWindowBuilder::from_config(app, &conf)?.visible(true).build()
}

/// Open a specific thread (from a toast click). The UI either receives the
/// event (window alive) or pulls it with `take_pending_open` on mount.
// macOS banner notifications have no button routing yet, so this is only
// called from the Windows and Linux notification paths.
#[cfg_attr(target_os = "macos", allow(dead_code))]
pub fn open_thread(app: &AppHandle, account_id: i64, thread_id: &str, message_id: Option<&str>) {
    let target = OpenTarget {
        account_id,
        thread_id: thread_id.into(),
        message_id: message_id.map(Into::into),
    };
    if let Some(state) = app.try_state::<AppState>() {
        *state.pending_open.lock().unwrap() = Some(target.clone());
    }
    show_main(app);
    let _ = app.emit_to(MAIN, "open-thread", target);
}

pub fn on_window_event(window: &Window, event: &WindowEvent) {
    if window.label() != MAIN {
        return;
    }
    if let WindowEvent::CloseRequested { api, .. } = event {
        let destroy = window
            .app_handle()
            .try_state::<AppState>()
            .map(|s| s.destroy_on_close.load(Ordering::Relaxed))
            .unwrap_or(false);
        if !destroy {
            api.prevent_close();
            let _ = window.hide();
        }
        // destroy: let it close; RunEvent::ExitRequested keeps the app alive.
    }
}
