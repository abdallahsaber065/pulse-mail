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

/// "off" plus every half hour of the day as minutes since midnight.
const QUIET_STEPS: &[&str] = &[
    "off", "0", "30", "60", "90", "120", "150", "180", "210", "240", "270", "300", "330", "360", "390", "420", "450",
    "480", "510", "540", "570", "600", "630", "660", "690", "720", "750", "780", "810", "840", "870", "900", "930",
    "960", "990", "1020", "1050", "1080", "1110", "1140", "1170", "1200", "1230", "1260", "1290", "1320", "1350",
    "1380", "1410",
];

pub const PREFS: &[Pref] = &[
    // Remote images in mail: ask (default, shows a bar) or always load (proxied).
    Pref {
        key: "images",
        default: "ask",
        allowed: &["ask", "always"],
    },
    // Dark theme rendering of HTML mail: smart invert or the sender's colors.
    Pref {
        key: "email_dark",
        default: "smart",
        allowed: &["smart", "original"],
    },
    // Copy a newly arrived verification code to the clipboard automatically.
    Pref {
        key: "otp_autocopy",
        default: "0",
        allowed: &["0", "1"],
    },
    // Clear a code we copied after this many seconds (if still on the clipboard).
    Pref {
        key: "clipboard_clear_secs",
        default: "60",
        allowed: &["0", "30", "60", "120"],
    },
    // Keep codes out of toasts while the PC is locked.
    Pref {
        key: "otp_lockscreen",
        default: "hide",
        allowed: &["hide", "show"],
    },
    // Delay before a sent message actually leaves (Undo window).
    Pref {
        key: "undo_send_secs",
        default: "5",
        allowed: &["0", "5", "10", "20", "30"],
    },
    // Local cache budget in MB (mail + saved attachments); downloads stop at it.
    Pref {
        key: "storage_budget_mb",
        default: "1024",
        allowed: &["512", "1024", "2048", "5120"],
    },
    // Quiet hours: suppress non-code toasts inside this local-time window
    // (minutes since midnight, 30-minute steps; "off" disables). The window
    // may cross midnight (e.g. 1320 -> 390 for 10 PM - 6:30 AM).
    Pref {
        key: "quiet_start",
        default: "off",
        allowed: QUIET_STEPS,
    },
    Pref {
        key: "quiet_end",
        default: "off",
        allowed: QUIET_STEPS,
    },
    // Verification codes are time-critical: they stay allowed to toast.
    Pref {
        key: "quiet_allow_otp",
        default: "1",
        allowed: &["0", "1"],
    },
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

/// True when `now_min` (minutes since local midnight) falls inside the quiet
/// window. Start is inclusive, end exclusive; a start later than the end
/// crosses midnight (22:00 -> 06:30). "off" or equal times disable it.
pub fn quiet_active(start: &str, end: &str, now_min: u32) -> bool {
    let (Some(s), Some(e)) = (parse_quiet(start), parse_quiet(end)) else {
        return false;
    };
    if s == e {
        return false;
    }
    if s < e {
        now_min >= s && now_min < e
    } else {
        now_min >= s || now_min < e
    }
}

fn parse_quiet(v: &str) -> Option<u32> {
    v.parse().ok().filter(|m| *m < 1440)
}

/// Minutes since local midnight right now. Windows: the timezone bias from
/// the OS; elsewhere (dev/CI builds, where toasts don't exist) UTC.
pub fn local_minutes_now() -> u32 {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let minutes = (secs % 86_400) / 60 + local_utc_offset_minutes();
    (minutes % 1440) as u32
}

/// Local time minus UTC in minutes for right now (includes active DST).
#[cfg(windows)]
fn local_utc_offset_minutes() -> u64 {
    use windows_sys::Win32::System::Time::{
        DYNAMIC_TIME_ZONE_INFORMATION, GetDynamicTimeZoneInformation, TIME_ZONE_INFORMATION,
    };
    const TIME_ZONE_ID_DAYLIGHT: u32 = 2;
    const TIME_ZONE_ID_INVALID: u32 = 0xFFFF_FFFF;
    // SAFETY: plain Win32 query; the struct is an out-parameter.
    let mut dynamic = unsafe { std::mem::zeroed::<DYNAMIC_TIME_ZONE_INFORMATION>() };
    let kind = unsafe { GetDynamicTimeZoneInformation(&mut dynamic) };
    // The dynamic struct starts with the same layout as TIME_ZONE_INFORMATION.
    let tz = unsafe { &*(&dynamic as *const _ as *const TIME_ZONE_INFORMATION) };
    let extra = if kind == TIME_ZONE_ID_DAYLIGHT {
        tz.DaylightBias
    } else {
        tz.StandardBias
    };
    let bias = if kind == TIME_ZONE_ID_INVALID {
        0
    } else {
        tz.Bias + extra
    };
    let offset = -i64::from(bias);
    offset.rem_euclid(1440) as u64
}

#[cfg(not(windows))]
fn local_utc_offset_minutes() -> u64 {
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_hours_windows() {
        // Off by default and on garbage.
        assert!(!quiet_active("off", "390", 500));
        assert!(!quiet_active("1320", "off", 500));
        assert!(!quiet_active("nope", "390", 500));
        // Same-day window: [08:00, 12:30)
        assert!(quiet_active("480", "750", 480), "start is inclusive");
        assert!(quiet_active("480", "750", 600));
        assert!(!quiet_active("480", "750", 750), "end is exclusive");
        assert!(!quiet_active("480", "750", 479));
        // Crossing midnight: [22:00, 06:30)
        assert!(quiet_active("1320", "390", 1320));
        assert!(quiet_active("1320", "390", 1439));
        assert!(quiet_active("1320", "390", 0), "midnight is inside");
        assert!(quiet_active("1320", "390", 300));
        assert!(!quiet_active("1320", "390", 390));
        assert!(!quiet_active("1320", "390", 700));
        // Equal times disable the window.
        assert!(!quiet_active("600", "600", 600));
        // All stored values stay parseable.
        for p in PREFS {
            if p.key.starts_with("quiet_") && p.default != "off" {
                assert!(parse_quiet(p.default).is_some(), "{}", p.key);
            }
        }
        for v in QUIET_STEPS {
            assert!(*v == "off" || (parse_quiet(v).is_some() && parse_quiet(v) != Some(1440)));
        }
    }

    #[test]
    fn local_minutes_is_sane() {
        let m = local_minutes_now();
        assert!(m < 1440, "minutes since midnight must be < 1440, got {m}");
    }

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
        assert_eq!(
            get(&s, "images"),
            "ask",
            "invalid stored values fall back to the default"
        );
        for p in PREFS {
            assert!(p.allowed.contains(&p.default), "{} default must be allowed", p.key);
        }
    }
}
