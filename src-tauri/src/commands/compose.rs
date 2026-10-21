//! Compose IPC: reply/forward context, Gmail drafts (create/update/delete)
//! and sending with an undo window. Message content is never logged.

use super::{CmdError, CmdResult, engine_for, resolve_account, validate};
use crate::AppState;
use crate::compose::{self, ComposeInit, Draft, FilePart, Mode};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

/// Sends waiting out their undo window: id → account.
static OUTBOX: LazyLock<Mutex<HashMap<u64, i64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_SEND: AtomicU64 = AtomicU64::new(1);

/// Files picked or dropped in compose, held in memory until the app quits
/// (the Gmail draft holds them too once saved): id -> file.
static STAGED: LazyLock<Mutex<HashMap<String, Staged>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// Cap on staged bytes kept in memory.
const STAGED_MAX: usize = 300 * 1024 * 1024;

struct Staged {
    name: String,
    mime: String,
    bytes: std::sync::Arc<Vec<u8>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StagedRef {
    id: String,
    name: String,
    mime: String,
    size: i64,
}

fn header<'a>(request: &'a tauri::ipc::Request<'_>, name: &str) -> Option<&'a str> {
    request.headers().get(name).and_then(|v| v.to_str().ok())
}

/// Percent-decoding for the file name header (the UI encodes it).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Receive one file for the compose window as a raw IPC body (no base64).
/// Headers: `x-name` (percent-encoded file name), `x-mime`.
#[tauri::command]
pub fn stage_attachment(request: tauri::ipc::Request<'_>) -> CmdResult<StagedRef> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(CmdError::new("Couldn't read that file."));
    };
    if bytes.len() as i64 > compose::MAX_ATTACH_TOTAL {
        return Err(CmdError::new("Files can be up to 25 MB (Gmail's limit)."));
    }
    let name = crate::sync::mime::safe_filename(&percent_decode(header(&request, "x-name").unwrap_or("attachment")));
    let name = if name.is_empty() { "attachment".to_string() } else { name };
    let mime = compose::safe_mime(header(&request, "x-mime").unwrap_or(""));
    let mut staged = STAGED.lock().unwrap_or_else(|e| e.into_inner());
    let used: usize = staged.values().map(|s| s.bytes.len()).sum();
    if used + bytes.len() > STAGED_MAX {
        return Err(CmdError::new("Too many large files are open in compose. Send or discard a message first."));
    }
    let id = format!("s{}", NEXT_SEND.fetch_add(1, Ordering::SeqCst));
    let size = bytes.len() as i64;
    staged.insert(id.clone(), Staged { name: name.clone(), mime: mime.clone(), bytes: std::sync::Arc::new(bytes.clone()) });
    Ok(StagedRef { id, name, mime, size })
}

/// Forget a file removed from compose.
#[tauri::command]
pub fn unstage_attachment(id: String) {
    STAGED.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
}

/// Bytes for every attachment of a draft: staged files from memory,
/// forwarded ones from this PC's attachment store or from Gmail.
async fn resolve_files(state: &AppState, d: &Draft, account_id: i64) -> CmdResult<Vec<FilePart>> {
    let mut out = Vec::new();
    let mut raws: HashMap<String, Vec<u8>> = HashMap::new();
    for a in &d.attachments {
        if let Some(id) = &a.staged {
            let s = STAGED.lock().unwrap_or_else(|e| e.into_inner());
            let f = s.get(id).ok_or_else(|| CmdError::new(format!("\"{}\" isn't available any more. Attach it again.", a.name)))?;
            out.push(FilePart { name: f.name.clone(), mime: f.mime.clone(), bytes: f.bytes.as_ref().clone() });
            continue;
        }
        let (Some(mid), Some(part)) = (&a.source_message, a.source_part) else { continue };
        validate::gmail_id(mid)?;
        let stored = {
            let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
            store.attachment_stored(account_id, mid, part)?
        };
        let local = if stored { crate::files::read(&state.files_dir, account_id, mid, part) } else { None };
        let bytes = match local {
            Some(b) => b,
            None => {
                if !raws.contains_key(mid) {
                    let engine = engine_for(state, Some(account_id)).await?;
                    raws.insert(mid.clone(), engine.fetch_raw(mid).await?);
                }
                mail_parser::MessageParser::default()
                    .parse(&raws[mid])
                    .and_then(|p| p.attachment(part).map(|x| x.contents().to_vec()))
                    .ok_or_else(|| CmdError::new(format!("Couldn't attach \"{}\" from the original.", a.name)))?
            }
        };
        out.push(FilePart { name: a.name.clone(), mime: a.mime.clone(), bytes });
    }
    Ok(out)
}

fn check_ids(d: &Draft) -> CmdResult<()> {
    if let Some(id) = &d.draft_id {
        validate::gmail_id(id)?;
    }
    if let Some(id) = &d.thread_id {
        validate::gmail_id(id)?;
    }
    validate::text(&d.in_reply_to, 1_000)?;
    validate::text(&d.references, 20_000)?;
    Ok(())
}

/// Prefill a reply, reply-all or forward of `message_id`.
#[tauri::command]
pub async fn compose_context(
    state: State<'_, AppState>,
    message_id: String,
    mode: Mode,
    account_id: Option<i64>,
) -> CmdResult<ComposeInit> {
    validate::gmail_id(&message_id)?;
    let account = resolve_account(&state, account_id)?;
    let thread = {
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        store.message_thread(account.id, &message_id)?
    }
    .ok_or_else(|| CmdError::new("That message isn't available."))?;
    // Replies need the full headers (Reply-To, Cc, Message-ID) and text.
    if let Ok(engine) = engine_for(&state, Some(account.id)).await
        && let Err(e) = engine.load_thread_bodies(&thread).await
    {
        tracing::warn!(error = %e, "compose: couldn't load the original");
    }
    let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
    let m = store
        .get_thread(account.id, &thread)?
        .into_iter()
        .find(|m| m.id == message_id)
        .ok_or_else(|| CmdError::new("That message isn't available."))?;
    Ok(compose::reply_context(&m, &account.email, mode))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Saved {
    draft_id: String,
    thread_id: Option<String>,
}

async fn save(state: &AppState, d: &Draft, account_id: Option<i64>, sending: bool) -> CmdResult<(Saved, i64)> {
    check_ids(d)?;
    let p = d.prepare(sending).map_err(CmdError::new)?;
    let account = resolve_account(state, account_id)?;
    let engine = engine_for(state, Some(account.id)).await?;
    let files = resolve_files(state, d, account.id).await?;
    let raw = compose::build_raw(d, &p, &account.email, &account.display_name, files);
    let r = engine
        .with_token(|t| {
            let (raw, engine) = (&raw, &engine);
            async move { engine.gmail().save_draft(&t, d.draft_id.as_deref(), raw, d.thread_id.as_deref()).await }
        })
        .await?;
    Ok((Saved { draft_id: r.id, thread_id: r.message.thread_id }, account.id))
}

/// Create or update the Gmail draft for an open compose window.
#[tauri::command]
pub async fn save_draft(state: State<'_, AppState>, draft: Draft, account_id: Option<i64>) -> CmdResult<Saved> {
    Ok(save(&state, &draft, account_id, false).await?.0)
}

/// Discard: delete the draft this compose window created.
#[tauri::command]
pub async fn delete_draft(state: State<'_, AppState>, draft_id: String, account_id: Option<i64>) -> CmdResult<()> {
    validate::gmail_id(&draft_id)?;
    let engine = engine_for(&state, account_id).await?;
    engine
        .with_token(|t| {
            let (id, engine) = (&draft_id, &engine);
            async move { engine.gmail().delete_draft(&t, id).await }
        })
        .await?;
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Sending {
    send_id: u64,
    draft_id: String,
    delay_secs: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SendResult {
    send_id: u64,
    /// "sent" | "failed"
    status: &'static str,
    message: Option<String>,
}

/// Save the draft, then send it after the undo window (`undo_send_secs`).
/// The draft stays in Gmail Drafts until it's sent, so quitting during the
/// window loses nothing.
#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    draft: Draft,
    account_id: Option<i64>,
) -> CmdResult<Sending> {
    let (saved, account) = save(&state, &draft, account_id, true).await?;
    let delay = {
        let store = state.store.lock().unwrap_or_else(|e| e.into_inner());
        crate::prefs::get_num(&store, "undo_send_secs")
    };
    let send_id = NEXT_SEND.fetch_add(1, Ordering::SeqCst);
    OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).insert(send_id, account);
    let draft_id = saved.draft_id.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(delay)).await;
        // Taking the entry out is the point of no return; Undo can't win after it.
        if OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).remove(&send_id).is_none() {
            return;
        }
        let state = app.state::<AppState>();
        let result = match engine_for(&state, Some(account)).await {
            Ok(engine) => engine
                .with_token(|t| {
                    let (id, engine) = (&draft_id, &engine);
                    async move { engine.gmail().send_draft(&t, id).await }
                })
                .await
                .map_err(|e| e.friendly()),
            Err(e) => Err(e.message),
        };
        let event = match result {
            Ok(_) => {
                tracing::info!("message sent");
                {
                    let mut staged = STAGED.lock().unwrap_or_else(|e| e.into_inner());
                    for a in &draft.attachments {
                        if let Some(id) = &a.staged {
                            staged.remove(id);
                        }
                    }
                }
                if let Some(c) = state.syncs.lock().await.get(&account) {
                    c.sync_now(false);
                }
                SendResult { send_id, status: "sent", message: None }
            }
            Err(e) => {
                tracing::warn!("send failed");
                SendResult { send_id, status: "failed", message: Some(e) }
            }
        };
        let _ = app.emit("send-result", event);
    });
    Ok(Sending { send_id, draft_id: saved.draft_id, delay_secs: delay })
}

/// Undo a send still inside its window. False if it already went out.
#[tauri::command]
pub fn cancel_send(send_id: u64) -> bool {
    OUTBOX.lock().unwrap_or_else(|e| e.into_inner()).remove(&send_id).is_some()
}
