//! System tray: left click opens the window, menu offers Sync now / Quit.
//! Also keeps the unread badge (tray dot + tooltip, taskbar overlay) current.

use crate::AppState;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Listener, Manager};

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Pulse Mail", true, None::<&str>)?;
    let sync = MenuItem::with_id(app, "sync", "Check mail now", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &sync, &sep, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("Pulse Mail")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => crate::window::show_main(app),
            "sync" => sync_now(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                crate::window::show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;

    // Refresh the badge whenever mail changes (debounced), and once at start.
    let h = app.clone();
    app.listen("mail-updated", move |_| schedule_badge(&h));
    schedule_badge(app);
    Ok(())
}

static BADGE_PENDING: AtomicBool = AtomicBool::new(false);

/// Coalesce bursts of `mail-updated` into one badge refresh 400 ms later.
pub fn schedule_badge(app: &AppHandle) {
    if BADGE_PENDING.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        BADGE_PENDING.store(false, Ordering::SeqCst);
        update_badge(&app);
    });
}

fn update_badge(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let (counts, accounts) = {
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        (
            store.inbox_unread().unwrap_or_default(),
            store.accounts().unwrap_or_default(),
        )
    };
    let total: i64 = counts.iter().map(|(_, n)| n).sum();
    let per: Vec<(String, i64)> = accounts
        .iter()
        .map(|a| {
            (
                a.email.clone(),
                counts.iter().find(|(id, _)| *id == a.id).map(|c| c.1).unwrap_or(0),
            )
        })
        .collect();

    if let Some(tray) = app.tray_by_id("main") {
        let _ = tray.set_tooltip(Some(crate::badge::tooltip(total, &per)));
        if let Some(icon) = app.default_window_icon() {
            let img = if total > 0 {
                let rgba = crate::badge::with_dot(icon.rgba(), icon.width(), icon.height());
                Image::new_owned(rgba, icon.width(), icon.height())
            } else {
                icon.clone()
            };
            let _ = tray.set_icon(Some(img));
        }
    }
    #[cfg(target_os = "windows")]
    if let Some(win) = app.get_webview_window("main") {
        let overlay = crate::badge::overlay(total).map(|(rgba, w, h)| Image::new_owned(rgba, w, h));
        let _ = win.set_overlay_icon(overlay);
    }
    #[cfg(target_os = "macos")]
    if let Some(win) = app.get_webview_window("main") {
        let count = if total > 0 { Some(total) } else { None };
        let _ = win.set_badge_count(count);
    }
}

fn sync_now(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        for c in state.syncs.lock().await.values() {
            c.sync_now(true);
        }
    });
}
