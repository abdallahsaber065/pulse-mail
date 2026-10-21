//! Compose IPC: reply/forward context, Gmail drafts (create/update/delete)
//! and sending with an undo window. Message content is never logged.

use super::{CmdError, CmdResult, engine_for, resolve_account, validate};
use crate::AppState;
use crate::compose::{self, ComposeInit, Draft, Mode};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

/// Sends waiting out their undo window: id → account.
static OUTBOX: LazyLock<Mutex<HashMap<u64, i64>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static NEXT_SEND: AtomicU64 = AtomicU64::new(1);

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
    let raw = compose::build_raw(d, &p, &account.email);
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
