//! Reader commands: safe rendering, remote images, attachments, prefs.

use super::{CmdError, CmdResult, validate};
use crate::AppState;
use crate::prefs;
use crate::render::{self, Rendered, RenderOptions, Resources};
use base64::Engine as _;
use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt as _;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderedMessage {
    #[serde(flatten)]
    rendered: Rendered,
    sender: String,
    /// The sender is on the "always show images" list.
    sender_allowed: bool,
}

fn account_id(state: &AppState) -> CmdResult<i64> {
    state
        .store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .primary_account()?
        .map(|a| a.id)
        .ok_or_else(|| CmdError::new("Not signed in."))
}

/// Sanitize and render one cached message for the reader's sandboxed frame.
/// Remote images are only fetched (through the Rust proxy) when the user
/// asked, the sender is allowlisted, or the "images" pref is "always".
#[tauri::command]
pub async fn render_message(
    state: State<'_, AppState>,
    message_id: String,
    dark: bool,
    show_images: bool,
) -> CmdResult<RenderedMessage> {
    validate::gmail_id(&message_id)?;
    let account = account_id(&state)?;
    let (from, html, text, inline, always, original, allowed) = {
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        let Some((from, html, text, _)) = store.message_body(account, &message_id)? else {
            return Err(CmdError::new("That message isn't available."));
        };
        let inline = store.inline_images(account, &message_id)?;
        let allowed = !from.is_empty() && store.images_allowed_from(account, &from)?;
        (
            from,
            html,
            text,
            inline,
            prefs::get(&store, "images") == "always",
            prefs::get(&store, "email_dark") == "original",
            allowed,
        )
    };
    let opts = RenderOptions {
        dark,
        original_colors: original,
    };
    let rendered = match html {
        Some(html) => {
            let mut res = Resources::default();
            for (cid, mime, data) in inline {
                let b64 = base64::engine::general_purpose::STANDARD.encode(&data);
                res.cids.insert(cid, format!("data:{mime};base64,{b64}"));
            }
            if show_images || allowed || always {
                let scan = render::scan(&html);
                if !scan.remote.is_empty() {
                    res.images = state.images.fetch_all(&scan.remote).await;
                }
            }
            tauri::async_runtime::spawn_blocking(move || render::render_html(&html, res, opts))
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "rendering a message failed");
                    CmdError::new("Couldn't display this message.")
                })?
        }
        None => render::render_text(&text, opts),
    };
    Ok(RenderedMessage {
        rendered,
        sender: from,
        sender_allowed: allowed,
    })
}

/// Add/remove a sender from the "always show images" list.
#[tauri::command]
pub fn set_image_sender(state: State<'_, AppState>, email: String, allowed: bool) -> CmdResult<()> {
    validate::email(&email)?;
    let account = account_id(&state)?;
    state
        .store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .set_images_allowed(account, &email, allowed)?;
    Ok(())
}

/// Change a backend pref (see prefs.rs for keys and allowed values).
#[tauri::command]
pub fn set_pref(state: State<'_, AppState>, key: String, value: String) -> CmdResult<()> {
    if !prefs::valid(&key, &value) {
        return Err(CmdError::new("Unknown setting."));
    }
    state
        .store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .set_setting(&key, &value)?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAttachment {
    path: String,
    opened: bool,
    /// Program-like file: saved but not opened (open it yourself if trusted).
    blocked: bool,
}

/// Extensions Windows will execute or that commonly carry malware.
const DANGEROUS: &[&str] = &[
    "exe", "com", "scr", "pif", "bat", "cmd", "ps1", "psm1", "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta", "msi",
    "msp", "msc", "cpl", "dll", "jar", "lnk", "reg", "url", "scf", "inf", "appx", "msix", "appinstaller", "iso",
    "img", "vhd", "vhdx", "application", "gadget", "chm", "settingcontent-ms", "xll", "library-ms",
];

pub fn is_dangerous(name: &str) -> bool {
    name.rsplit_once('.')
        .map(|(_, ext)| DANGEROUS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// Download one attachment (re-fetching the message from Gmail), then either
/// open it with the default app (`open = true`, from a private cache folder)
/// or save it to Downloads and show it in Explorer. Files get the Windows
/// "downloaded from the internet" mark, and program-like files are never
/// opened directly.
#[tauri::command]
pub async fn save_attachment(
    app: AppHandle,
    state: State<'_, AppState>,
    message_id: String,
    part: u32,
    open: bool,
) -> CmdResult<SavedAttachment> {
    validate::gmail_id(&message_id)?;
    let account = account_id(&state)?;
    let meta = state
        .store
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .attachments(account, &message_id, true)?
        .into_iter()
        .find(|a| a.part == part)
        .ok_or_else(|| CmdError::new("That attachment isn't available."))?;
    let engine = state
        .sync
        .lock()
        .await
        .as_ref()
        .map(|c| c.engine())
        .ok_or_else(|| CmdError::new("Not signed in."))?;
    let raw = engine.fetch_raw(&message_id).await?;
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        mail_parser::MessageParser::default()
            .parse(&raw)
            .and_then(|p| p.attachment(part).map(|a| a.contents().to_vec()))
    })
    .await
    .ok()
    .flatten()
    .ok_or_else(|| CmdError::new("Couldn't read that attachment."))?;

    let name = crate::sync::mime::safe_filename(&meta.filename);
    let dir = if open {
        app.path().app_cache_dir()?.join("attachments").join(&message_id)
    } else {
        app.path().download_dir()?
    };
    std::fs::create_dir_all(&dir).map_err(io_err)?;
    let path = unique_path(&dir, &name);
    std::fs::write(&path, &bytes).map_err(io_err)?;
    mark_of_the_web(&path);
    tracing::info!(bytes = bytes.len(), open, "attachment saved");

    let blocked = is_dangerous(&name);
    let opened = if open && !blocked {
        app.opener().open_path(path.display().to_string(), None::<&str>)?;
        true
    } else {
        app.opener().reveal_item_in_dir(&path)?;
        false
    };
    Ok(SavedAttachment {
        path: path.display().to_string(),
        opened,
        blocked: open && blocked,
    })
}

fn io_err(e: std::io::Error) -> CmdError {
    tracing::warn!(error = %e, "attachment write failed");
    CmdError::new("Couldn't save the file.")
}

/// `name.ext`, `name (1).ext`, ... — never overwrite an existing file.
pub fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    if !p.exists() {
        return p;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
        _ => (name.to_string(), String::new()),
    };
    (1..1000)
        .map(|i| dir.join(format!("{stem} ({i}){ext}")))
        .find(|p| !p.exists())
        .unwrap_or_else(|| dir.join(format!("{stem}-{}{ext}", crate::sync::now_ms())))
}

/// Zone.Identifier stream: Windows (SmartScreen, Office Protected View)
/// treats the file as downloaded from the internet.
fn mark_of_the_web(path: &Path) {
    #[cfg(windows)]
    {
        let ads = format!("{}:Zone.Identifier", path.display());
        if let Err(e) = std::fs::write(ads, "[ZoneTransfer]\r\nZoneId=3\r\n") {
            tracing::debug!(error = %e, "could not write mark of the web");
        }
    }
    #[cfg(not(windows))]
    let _ = path;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dangerous_extensions() {
        assert!(is_dangerous("invoice.PDF.exe"));
        assert!(is_dangerous("x.js") && is_dangerous("a.lnk") && is_dangerous("b.iso"));
        assert!(!is_dangerous("report.pdf") && !is_dangerous("photo.jpg") && !is_dangerous("noext"));
    }

    #[test]
    fn unique_paths_never_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let a = unique_path(dir.path(), "r.pdf");
        std::fs::write(&a, b"1").unwrap();
        let b = unique_path(dir.path(), "r.pdf");
        assert_eq!(b.file_name().unwrap(), "r (1).pdf");
    }
}
