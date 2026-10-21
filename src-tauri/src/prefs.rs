//! Backend-owned settings (stored in the `settings` table). Every key has a
//! fixed set of allowed values, so the IPC `set_pref` command can't be used to
//! write arbitrary rows. UI-only preferences (density, theme, reading pane)
//! live in the web UI's localStorage instead.

use crate::store::Store;
use std::collections::BTreeMap;

pub struct Pref {
    pub key: &'static str,
    pub default: &'static str,
    pub allowed: &'static [&'static str],
}

pub const PREFS: &[Pref] = &[
    // Remote images in mail: ask (default, shows a bar) or always load (proxied).
    Pref { key: "images", default: "ask", allowed: &["ask", "always"] },
    // Dark theme rendering of HTML mail: smart invert or the sender's colors.
    Pref { key: "email_dark", default: "smart", allowed: &["smart", "original"] },
    // Copy a newly arrived verification code to the clipboard automatically.
    Pref { key: "otp_autocopy", default: "0", allowed: &["0", "1"] },
    // Clear a code we copied after this many seconds (if still on the clipboard).
    Pref { key: "clipboard_clear_secs", default: "60", allowed: &["0", "30", "60", "120"] },
    // Keep codes out of toasts while the PC is locked.
    Pref { key: "otp_lockscreen", default: "hide", allowed: &["hide", "show"] },
    // Delay before a sent message actually leaves (Undo window).
    Pref { key: "undo_send_secs", default: "5", allowed: &["0", "5", "10", "20", "30"] },
    // Local cache budget in MB (mail + saved attachments); downloads stop at it.
    Pref { key: "storage_budget_mb", default: "1024", allowed: &["512", "1024", "2048", "5120"] },
];

pub fn find(key: &str) -> Option<&'static Pref> {
    PREFS.iter().find(|p| p.key == key)
}

/// True if `key` is a known pref and `value` one of its allowed values.
pub fn valid(key: &str, value: &str) -> bool {
    find(key).is_some_and(|p| p.allowed.contains(&value))
}

/// Current value (stored or default). Unknown keys return "".
pub fn get(store: &Store, key: &str) -> String {
    let Some(p) = find(key) else { return String::new() };
    match store.get_setting(key) {
        Ok(Some(v)) if p.allowed.contains(&v.as_str()) => v,
        _ => p.default.to_string(),
    }
}

pub fn get_num(store: &Store, key: &str) -> u64 {
    get(store, key).parse().unwrap_or(0)
}

pub fn all(store: &Store) -> BTreeMap<&'static str, String> {
    PREFS.iter().map(|p| (p.key, get(store, p.key))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_and_validation() {
        let s = Store::open_in_memory().unwrap();
        assert_eq!(get(&s, "images"), "ask");
        assert_eq!(get(&s, "otp_autocopy"), "0");
        assert_eq!(get_num(&s, "clipboard_clear_secs"), 60);
        assert!(valid("images", "always"));
        assert!(!valid("images", "sometimes"));
        assert!(!valid("history_id", "1"));
        s.set_setting("images", "bogus").unwrap();
        assert_eq!(get(&s, "images"), "ask", "invalid stored values fall back to the default");
        for p in PREFS {
            assert!(p.allowed.contains(&p.default), "{} default must be allowed", p.key);
        }
    }
}
