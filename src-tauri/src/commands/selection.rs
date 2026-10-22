//! Frozen bulk selections live in Rust; the renderer gets a count and a token.
use super::{CmdError, CmdResult, engine_for, validate};
use crate::{AppState, store::selection::ThreadKey};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::{
    Mutex,
    atomic::{AtomicU64, Ordering},
};
use tauri::{AppHandle, Emitter, State};

const BATCH: usize = 10;
const MAX_KEYS: usize = 5000;

#[derive(Default)]
pub struct Selections {
    next: AtomicU64,
    snapshot: Mutex<Option<Snapshot>>,
    writing: tokio::sync::Mutex<()>,
}

struct Snapshot {
    id: u64,
    keys: Vec<ThreadKey>,
    excluded: BTreeSet<ThreadKey>,
}

impl Selections {
    fn reserve(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed) + 1
    }

    fn replace(&self, id: u64, mut keys: Vec<ThreadKey>) -> CmdResult<SelectionInfo> {
        keys.sort_unstable();
        keys.dedup();
        let total = keys.len();
        let mut snapshot = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        if self.next.load(Ordering::Relaxed) != id {
            return Err(CmdError::new(
                "This selection has changed. Select the conversations again.",
            ));
        }
        *snapshot = Some(Snapshot {
            id,
            keys,
            excluded: BTreeSet::new(),
        });
        Ok(SelectionInfo { id, total })
    }

    fn with<T>(&self, id: u64, f: impl FnOnce(&[ThreadKey]) -> T) -> CmdResult<T> {
        let guard = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        let snapshot = guard
            .as_ref()
            .filter(|s| s.id == id)
            .ok_or_else(|| CmdError::new("This selection has changed. Select the conversations again."))?;
        Ok(f(&snapshot.keys))
    }

    fn batch(&self, id: u64, offset: usize, excluded: Vec<ThreadKey>) -> CmdResult<(Vec<ThreadKey>, usize, usize)> {
        let mut guard = self.snapshot.lock().unwrap_or_else(|e| e.into_inner());
        let snapshot = guard
            .as_mut()
            .filter(|s| s.id == id)
            .ok_or_else(|| CmdError::new("This selection has changed. Select the conversations again."))?;
        if offset == 0 {
            snapshot.excluded = excluded.into_iter().collect();
        }
        let end = offset.saturating_add(BATCH).min(snapshot.keys.len());
        let keys = snapshot
            .keys
            .get(offset..end)
            .unwrap_or_default()
            .iter()
            .filter(|k| !snapshot.excluded.contains(k))
            .cloned()
            .collect();
        Ok((keys, end, snapshot.keys.len()))
    }
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SelectionSource {
    View {
        account_id: Option<i64>,
        label_id: String,
        category: Option<String>,
        query: String,
    },
    Threads {
        threads: Vec<ThreadKey>,
    },
}

#[derive(Serialize)]
pub struct SelectionInfo {
    id: u64,
    total: usize,
}

fn validate_keys(keys: &[ThreadKey]) -> CmdResult<()> {
    if keys.len() > MAX_KEYS {
        return Err(CmdError::new("Select fewer conversations and try again."));
    }
    for key in keys {
        if key.account_id <= 0 {
            return Err(CmdError::new("That account isn't available."));
        }
        validate::gmail_id(&key.thread_id)?;
    }
    Ok(())
}

#[tauri::command]
pub async fn create_selection(
    state: State<'_, AppState>,
    selections: State<'_, Selections>,
    source: SelectionSource,
) -> CmdResult<SelectionInfo> {
    let ticket = selections.reserve();
    let store = state.store.clone();
    let keys = match source {
        SelectionSource::Threads { threads } => {
            validate_keys(&threads)?;
            threads
        }
        SelectionSource::View {
            account_id,
            label_id,
            category,
            query,
        } => {
            validate::label_id(&label_id)?;
            validate::text(&query, 500)?;
            if category
                .as_ref()
                .is_some_and(|c| !crate::store::CATEGORIES.contains(&c.as_str()))
            {
                return Err(CmdError::new("Unknown inbox tab."));
            }
            if !query.trim().is_empty() && !crate::store::query::parse(&query).local_ok {
                return Err(CmdError::new("Select the loaded Gmail results for this search."));
            }
            tauri::async_runtime::spawn_blocking(move || {
                store.lock().unwrap_or_else(|e| e.into_inner()).selection_keys(
                    account_id,
                    &label_id,
                    category.as_deref(),
                    &query,
                )
            })
            .await
            .map_err(|_| CmdError::new("Couldn't select these conversations. Please try again."))??
        }
    };
    selections.replace(ticket, keys)
}

#[tauri::command]
pub fn selection_members(
    selections: State<'_, Selections>,
    selection_id: u64,
    threads: Vec<ThreadKey>,
) -> CmdResult<Vec<ThreadKey>> {
    validate_keys(&threads)?;
    selections.with(selection_id, |keys| {
        threads.into_iter().filter(|k| keys.binary_search(k).is_ok()).collect()
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadBatch {
    next_offset: usize,
    done: bool,
    changed: usize,
    failed: usize,
    error: Option<String>,
}

/// One small, sequential batch. The UI can stop between batches without
/// spawning a job or flooding Gmail; sync keeps running independently.
#[tauri::command]
pub async fn mark_selection_read(
    app: AppHandle,
    state: State<'_, AppState>,
    selections: State<'_, Selections>,
    selection_id: u64,
    offset: usize,
    excluded: Vec<ThreadKey>,
) -> CmdResult<ReadBatch> {
    validate_keys(&excluded)?;
    let _writer = selections.writing.lock().await;
    let (keys, end, total) = selections.batch(selection_id, offset, excluded)?;
    let start = std::time::Instant::now();
    let mut out = ReadBatch {
        next_offset: offset.min(total),
        done: false,
        changed: 0,
        failed: 0,
        error: None,
    };
    for key in keys {
        out.next_offset += 1;
        let unread = state
            .store
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .thread_has_unread(key.account_id, &key.thread_id)?;
        if !unread {
            continue;
        }
        let result = async {
            let engine = engine_for(&state, Some(key.account_id)).await?;
            engine.mark_thread_read(&key.thread_id).await?;
            Ok::<_, CmdError>(())
        }
        .await;
        match result {
            Ok(()) => out.changed += 1,
            Err(e) => {
                out.failed = 1;
                out.error = Some(e.message);
                break;
            }
        }
    }
    if out.failed == 0 {
        out.next_offset = end;
    }
    out.done = out.next_offset >= total;
    if out.changed > 0 {
        let _ = app.emit("mail-updated", ());
    }
    if out.changed > 0 {
        // At most ten writes per second for this user action, leaving room for sync.
        tokio::time::sleep(std::time::Duration::from_secs(1).saturating_sub(start.elapsed())).await;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshot_is_deduped_account_scoped_and_replaces_previous_selection() {
        let state = Selections::default();
        let k = |a| ThreadKey {
            account_id: a,
            thread_id: "same".into(),
        };
        let first = state.replace(state.reserve(), vec![k(1), k(2), k(1)]).unwrap();
        assert_eq!(first.total, 2);
        assert!(state.with(first.id, |keys| keys.binary_search(&k(2)).is_ok()).unwrap());
        let older = state.reserve();
        let next = state.replace(state.reserve(), vec![k(1)]).unwrap();
        assert!(state.replace(older, vec![k(2)]).is_err());
        assert!(state.with(first.id, |keys| keys.len()).is_err());
        assert_eq!(state.with(next.id, |keys| keys.len()).unwrap(), 1);
    }

    #[test]
    fn batches_preserve_exclusions_without_resending_them_and_bound_work() {
        let state = Selections::default();
        let key = |n| ThreadKey {
            account_id: 1,
            thread_id: format!("t{n:03}"),
        };
        let snapshot = state.replace(state.reserve(), (0..25).map(key).collect()).unwrap();
        let (first, offset, total) = state.batch(snapshot.id, 0, vec![key(3), key(12)]).unwrap();
        assert_eq!((first.len(), offset, total), (9, 10, 25));
        assert!(!first.contains(&key(3)));
        let (next, offset, _) = state.batch(snapshot.id, offset, vec![]).unwrap();
        assert_eq!((next.len(), offset), (9, 20));
        assert!(!next.contains(&key(12)));
        assert_eq!(state.batch(snapshot.id, 20, vec![]).unwrap().0.len(), 5);
        assert!(state.batch(snapshot.id, usize::MAX, vec![]).unwrap().0.is_empty());
    }
}
