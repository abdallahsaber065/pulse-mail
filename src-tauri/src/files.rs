//! Attachments kept on this PC: `<app data>/attachments/<account>/<message>/<part>`.
//!
//! Which files are kept is a per-account policy (none / up to N MB / all).
//! Paths are built only from numeric account ids, validated Gmail ids and
//! part numbers, never from file names in the mail.

use crate::store::{Account, Store};
use std::path::{Path, PathBuf};

/// Largest single file kept by "all" (Gmail's own limit is 25 MB per mail,
/// received mail can be bigger).
pub const MAX_FILE_BYTES: i64 = 50 * 1024 * 1024;

/// Per-file byte limit of the account's policy; None = keep no files.
pub fn limit(a: &Account) -> Option<i64> {
    match a.attach_mode.as_str() {
        "none" => None,
        "all" => Some(MAX_FILE_BYTES),
        _ => Some(a.attach_max_mb.clamp(1, 50) * 1024 * 1024),
    }
}

fn safe_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric())
}

pub fn path(base: &Path, account_id: i64, message_id: &str, part: u32) -> Option<PathBuf> {
    safe_id(message_id).then(|| {
        base.join(account_id.to_string())
            .join(message_id)
            .join(part.to_string())
    })
}

pub fn read(base: &Path, account_id: i64, message_id: &str, part: u32) -> Option<Vec<u8>> {
    std::fs::read(path(base, account_id, message_id, part)?).ok()
}

/// Remove every saved file of an account (on account removal).
pub fn remove_account(base: &Path, account_id: i64) {
    let _ = std::fs::remove_dir_all(base.join(account_id.to_string()));
}

/// Save the attachments of one downloaded message that the policy allows and
/// that aren't saved yet. Returns the bytes written.
pub fn save_from_raw(base: &Path, store: &std::sync::Mutex<Store>, a: &Account, message_id: &str, raw: &[u8]) -> i64 {
    let Some(max) = limit(a) else { return 0 };
    let wanted: Vec<u32> = {
        let s = store.lock().unwrap_or_else(|e| e.into_inner());
        s.attachments(a.id, message_id, false)
            .unwrap_or_default()
            .into_iter()
            .filter(|x| !x.stored && x.size <= max)
            .map(|x| x.part)
            .collect()
    };
    if wanted.is_empty() {
        return 0;
    }
    let Some(parsed) = mail_parser::MessageParser::default().parse(raw) else {
        return 0;
    };
    let mut written = 0;
    for part in wanted {
        let Some(att) = parsed.attachment(part) else {
            // Part was not present in raw MIME payload; mark stored to prevent infinite re-download loops
            let s = store.lock().unwrap_or_else(|e| e.into_inner());
            let _ = s.mark_stored(a.id, message_id, part);
            continue;
        };
        let Some(p) = path(base, a.id, message_id, part) else {
            let s = store.lock().unwrap_or_else(|e| e.into_inner());
            let _ = s.mark_stored(a.id, message_id, part);
            continue;
        };
        let bytes = att.contents();
        let ok = p.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&p, bytes).is_ok();
        if ok {
            let s = store.lock().unwrap_or_else(|e| e.into_inner());
            if s.mark_stored(a.id, message_id, part).is_ok() {
                written += bytes.len() as i64;
            }
        } else {
            tracing::warn!("couldn't save an attachment to disk");
            let s = store.lock().unwrap_or_else(|e| e.into_inner());
            let _ = s.mark_stored(a.id, message_id, part);
        }
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_never_use_untrusted_names() {
        let base = Path::new("/data/attachments");
        assert_eq!(
            path(base, 3, "18c2fa", 1).unwrap(),
            Path::new("/data/attachments/3/18c2fa/1")
        );
        assert!(path(base, 3, "../x", 1).is_none());
        assert!(path(base, 3, "a/b", 1).is_none());
        assert!(path(base, 3, "", 1).is_none());
    }

    #[test]
    fn policy_limits() {
        let mut a = Account {
            id: 1,
            email: "a@x".into(),
            history_id: None,
            last_sync_at: None,
            sync_window: "90d".into(),
            notify: "all".into(),
            backfill_state: "done".into(),
            backfill_page: None,
            backfill_done: 0,
            backfill_total: 0,
            messages_total: 0,
            backfill_phase: String::new(),
            backfill_error: None,
            attach_mode: "limit".into(),
            attach_max_mb: 10,
            display_name: String::new(),
            avatar_data: None,
            color: String::new(),
            signature: String::new(),
            category_tabs: true,
        };
        assert_eq!(limit(&a), Some(10 * 1024 * 1024));
        a.attach_mode = "none".into();
        assert_eq!(limit(&a), None);
        a.attach_mode = "all".into();
        assert_eq!(limit(&a), Some(MAX_FILE_BYTES));
    }
}
