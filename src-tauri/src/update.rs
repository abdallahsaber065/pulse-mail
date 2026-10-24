//! App self-update: check on startup (once, no polling loop) or on demand,
//! optional background download, and install with the user's click.
//! Artifacts come from this project's GitHub Releases (`latest.json`,
//! minisign-signed); that endpoint is the only non-Google network contact
//! the app makes, added on the maintainer's explicit request (ADR 0027).

use crate::commands::{CmdError, CmdResult};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

/// The discovered update, kept so it can be downloaded/installed later.
pub struct Pending(pub tokio::sync::Mutex<Option<tauri_plugin_updater::Update>>);
impl Default for Pending {
    fn default() -> Self {
        Self(tokio::sync::Mutex::new(None))
    }
}

/// Bytes of an update already downloaded in the background.
pub struct Downloaded(pub std::sync::Mutex<Option<Vec<u8>>>);
impl Default for Downloaded {
    fn default() -> Self {
        Self(std::sync::Mutex::new(None))
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAvail {
    pub version: String,
    pub notes: String,
}

/// Check for a newer release. `notify` emits UI events (startup path);
/// `auto_download` fetches the artifact in the background so "Install" is
/// instant, but never installs anything without the user's click.
pub async fn run_check(app: &AppHandle, auto_download: bool, notify: bool) -> Result<Option<UpdateAvail>, String> {
    let updater = app.updater_builder().build().map_err(|e| e.to_string())?;
    let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
        // A previous session may have left state behind; clear it.
        if let Some(d) = app.try_state::<Downloaded>() {
            *d.0.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        return Ok(None);
    };
    let info = UpdateAvail {
        version: update.version.clone(),
        notes: update.body.clone().unwrap_or_default(),
    };
    {
        let pending = app.state::<Pending>();
        *pending.0.lock().await = Some(update);
    }
    if auto_download {
        if notify {
            let _ = app.emit("update-downloading", &info);
        }
        download_pending(app).await?;
        if notify {
            let _ = app.emit("update-ready", &info);
        }
    } else if notify {
        let _ = app.emit("update-available", &info);
    }
    Ok(Some(info))
}

/// Download the pending update's artifact (holding the tokio mutex across
/// the await is the design: it serializes concurrent checks/installs).
async fn download_pending(app: &AppHandle) -> Result<(), String> {
    let pending = app.state::<Pending>();
    let guard = pending.0.lock().await;
    let Some(update) = guard.as_ref() else {
        return Err("No update is available.".into());
    };
    let bytes = update
        .download(|_chunk, _total| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    drop(guard);
    if let Some(d) = app.try_state::<Downloaded>() {
        *d.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(bytes);
    }
    Ok(())
}

/// Manual check from Settings. Returns the newer version, if any.
#[tauri::command]
pub async fn update_check(app: AppHandle) -> CmdResult<Option<UpdateAvail>> {
    let auto = {
        let Some(state) = app.try_state::<crate::AppState>() else {
            return Err(CmdError::new("The app isn't ready yet."));
        };
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        crate::prefs::get(&store, "update_auto_download") == "1"
    };
    match run_check(&app, auto, false).await {
        Ok(r) => Ok(r),
        Err(e) => {
            tracing::debug!(error = %e, "manual update check failed");
            Err(CmdError::new(
                "Couldn't reach the update server. Check your connection and try again.",
            ))
        }
    }
}

/// Install the update: uses the background-downloaded bytes when present,
/// otherwise downloads now, then launches the installer (Windows) or
/// replaces the app and restarts (macOS/Linux).
#[tauri::command]
pub async fn update_install(app: AppHandle) -> CmdResult<()> {
    let bytes = app
        .try_state::<Downloaded>()
        .and_then(|d| d.0.lock().unwrap_or_else(|e| e.into_inner()).take());
    let pending = app.state::<Pending>();
    let guard = pending.0.lock().await;
    let Some(update) = guard.as_ref() else {
        return Err(CmdError::new("No update is available yet."));
    };
    let outcome = match bytes {
        Some(bytes) => update.install(&bytes).map_err(|e| e.to_string()),
        None => update
            .download_and_install(|_chunk, _total| {}, || {})
            .await
            .map_err(|e| e.to_string()),
    };
    if let Err(e) = outcome {
        tracing::debug!(error = %e, "update install failed");
        return Err(CmdError::new(
            "The update couldn't be installed. Try downloading it from the Releases page.",
        ));
    }
    // Windows: the installer exits the app itself. macOS/Linux: swap and restart.
    #[cfg(not(windows))]
    app.restart();
    Ok(())
}
