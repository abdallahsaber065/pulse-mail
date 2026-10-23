//! Native Windows toasts. OTP mail gets a "Copy code" button that works
//! without opening the window; clicking the toast opens that exact thread.
#![allow(dead_code)]

use tauri::AppHandle;
#[cfg(windows)]
use tauri::Manager;

#[derive(Debug, Clone)]
pub struct NewMailToast {
    pub account_id: i64,
    /// Account email, shown when several accounts exist ("" = hide).
    pub account: String,
    pub message_id: String,
    pub thread_id: String,
    pub from: String,
    pub subject: String,
    pub snippet: String,
    pub otp: Option<String>,
    /// The mail says the code expires in N minutes.
    pub expires_min: Option<u32>,
    /// PC is locked and the user wants codes kept off the lock screen.
    pub hide_code: bool,
}

/// What the toast shows; pure so it can be unit-tested.
#[derive(Debug, PartialEq)]
pub struct ToastText {
    pub title: String,
    pub line1: String,
    pub line2: String,
}

pub fn toast_text(t: &NewMailToast) -> ToastText {
    let subject = if t.subject.trim().is_empty() {
        "(no subject)".to_string()
    } else {
        t.subject.clone()
    };
    let text = match &t.otp {
        Some(_) if t.hide_code => ToastText {
            title: format!("Verification code from {}", t.from),
            line1: subject,
            line2: "Unlock your PC to see it.".into(),
        },
        Some(code) => ToastText {
            title: format!("{code}  ·  {}", t.from),
            line1: subject,
            line2: match t.expires_min {
                Some(m) => format!("Verification code, expires in {m} min. Use \"Copy code\" to copy it."),
                None => "Verification code. Use \"Copy code\" to copy it.".into(),
            },
        },
        None => ToastText {
            title: t.from.clone(),
            line1: subject,
            line2: ellipsize(&t.snippet, 140),
        },
    };
    // With several accounts, say which inbox it landed in.
    if t.account.is_empty() {
        text
    } else {
        ToastText {
            line2: ellipsize(&format!("To {} · {}", t.account, text.line2), 140),
            ..text
        }
    }
}

fn ellipsize(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars - 1).collect();
    out.push('…');
    out
}

#[cfg(windows)]
pub fn init(app: &AppHandle) {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyW, RegSetValueExW, HKEY_CURRENT_USER, REG_SZ,
    };
    use windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;

    let app_id = &app.config().identifier;
    let wide_app_id: Vec<u16> = app_id.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(wide_app_id.as_ptr());
    }

    // Ensure icon exists in app_data_dir for toast display and registry icon uri
    let icon_path = if let Ok(data_dir) = app.path().app_data_dir() {
        let dest = data_dir.join("app-icon.png");
        if !dest.exists() {
            const ICON_BYTES: &[u8] = include_bytes!("../../icons/128x128.png");
            let _ = std::fs::write(&dest, ICON_BYTES);
        }
        dest
    } else {
        std::path::PathBuf::new()
    };

    let subkey = format!("SOFTWARE\\Classes\\AppUserModelId\\{app_id}\0");
    let wide_subkey: Vec<u16> = subkey.encode_utf16().collect();
    let mut hkey = std::ptr::null_mut();

    unsafe {
        if RegCreateKeyW(
            HKEY_CURRENT_USER,
            wide_subkey.as_ptr(),
            &mut hkey,
        ) == 0
            && !hkey.is_null()
        {
            let name: Vec<u16> = "Pulse Mail\0".encode_utf16().collect();
            let val_name: Vec<u16> = "DisplayName\0".encode_utf16().collect();
            let _ = RegSetValueExW(
                hkey,
                val_name.as_ptr(),
                0,
                REG_SZ,
                name.as_ptr() as *const u8,
                (name.len() * 2) as u32,
            );

            let bg: Vec<u16> = "0\0".encode_utf16().collect();
            let val_bg: Vec<u16> = "IconBackgroundColor\0".encode_utf16().collect();
            let _ = RegSetValueExW(
                hkey,
                val_bg.as_ptr(),
                0,
                REG_SZ,
                bg.as_ptr() as *const u8,
                (bg.len() * 2) as u32,
            );

            if let Some(s) = icon_path.to_str() {
                let uri: Vec<u16> = format!("{s}\0").encode_utf16().collect();
                let val_uri: Vec<u16> = "IconUri\0".encode_utf16().collect();
                let _ = RegSetValueExW(
                    hkey,
                    val_uri.as_ptr(),
                    0,
                    REG_SZ,
                    uri.as_ptr() as *const u8,
                    (uri.len() * 2) as u32,
                );
            }

            RegCloseKey(hkey);
        }
    }
}

#[cfg(not(windows))]
pub fn init(_app: &AppHandle) {}

#[cfg(windows)]
pub fn show_new_mail(app: &AppHandle, t: NewMailToast) {
    use tauri_winrt_notification::{Duration, IconCrop, Sound, Toast};

    let app_id = app.config().identifier.clone();
    let text = toast_text(&t);
    let app = app.clone();
    // WinRT calls can block briefly; keep them off the async runtime.
    std::thread::spawn(move || {
        // Initialize COM on this worker thread so WinRT calls run cleanly in MTA
        unsafe {
            windows_sys::Win32::System::Com::CoInitializeEx(
                std::ptr::null_mut(),
                windows_sys::Win32::System::Com::COINIT_MULTITHREADED as u32,
            );
        }

        let mut toast = Toast::new(&app_id)
            .title(&text.title)
            .text1(&text.line1)
            .text2(&text.line2)
            .sound(Some(Sound::Mail))
            .duration(if t.otp.is_some() {
                Duration::Long
            } else {
                Duration::Short
            });

        if let Ok(data_dir) = app.path().app_data_dir() {
            let icon_path = data_dir.join("app-icon.png");
            if icon_path.exists() {
                toast = toast.icon(&icon_path, IconCrop::Square, "Pulse Mail");
            }
        }

        if t.otp.is_some() {
            toast = toast.add_button("Copy code", "copy").add_button("Open", "open");
        }
        let otp = t.otp.clone();
        let thread_id = t.thread_id.clone();
        let message_id = t.message_id.clone();
        let account_id = t.account_id;

        let (tx, rx) = std::sync::mpsc::channel();
        let tx_dismiss = tx.clone();

        let result = toast
            .on_activated(move |action| {
                match (action.as_deref(), &otp) {
                    (Some("copy"), Some(code)) => crate::copy_secret(&app, code),
                    _ => crate::window::open_thread(&app, account_id, &thread_id, Some(&message_id)),
                }
                let _ = tx.send(());
                Ok(())
            })
            .on_dismissed(move |_| {
                let _ = tx_dismiss.send(());
                Ok(())
            })
            .show();
        match result {
            // Timestamped marker for measuring push-to-toast latency (PLAN.md).
            Ok(()) => {
                tracing::info!(otp = t.otp.is_some(), "toast shown");
                let timeout = if t.otp.is_some() {
                    std::time::Duration::from_secs(25)
                } else {
                    std::time::Duration::from_secs(10)
                };
                let _ = rx.recv_timeout(timeout);
            }
            Err(e) => tracing::warn!(error = %e, "failed to show toast"),
        }
    });
}

/// True while the workstation is locked (the input desktop is the secure
/// Winlogon desktop, which we can't open).
#[cfg(windows)]
pub fn session_locked() -> bool {
    use windows_sys::Win32::System::StationsAndDesktops::{CloseDesktop, OpenInputDesktop};
    const DESKTOP_SWITCHDESKTOP: u32 = 0x0100;
    // SAFETY: plain Win32 calls; the handle is closed right away.
    unsafe {
        let h = OpenInputDesktop(0, 0, DESKTOP_SWITCHDESKTOP);
        if h.is_null() {
            return true;
        }
        CloseDesktop(h);
        false
    }
}

#[cfg(not(windows))]
pub fn session_locked() -> bool {
    false
}

#[cfg(not(windows))]
pub fn show_new_mail(_app: &AppHandle, t: NewMailToast) {
    tracing::info!(thread = %t.thread_id, "new mail (toasts are Windows-only)");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(otp: Option<&str>) -> NewMailToast {
        NewMailToast {
            account_id: 1,
            account: String::new(),
            message_id: "m".into(),
            thread_id: "t".into(),
            from: "GitHub".into(),
            subject: "Please verify your device".into(),
            snippet: "x".repeat(300),
            otp: otp.map(Into::into),
            expires_min: None,
            hide_code: false,
        }
    }

    #[test]
    fn otp_toast_leads_with_code() {
        let tt = toast_text(&t(Some("731590")));
        assert!(tt.title.starts_with("731590"));
        assert!(tt.title.contains("GitHub"));
    }

    #[test]
    fn locked_screen_hides_the_code() {
        let mut x = t(Some("731590"));
        x.hide_code = true;
        let tt = toast_text(&x);
        assert!(!format!("{}{}{}", tt.title, tt.line1, tt.line2).contains("731590"));
        assert!(tt.title.contains("GitHub"));
    }

    #[test]
    fn regular_toast_truncates_snippet() {
        let tt = toast_text(&t(None));
        assert_eq!(tt.title, "GitHub");
        assert_eq!(tt.line2.chars().count(), 140);
        assert!(tt.line2.ends_with('…'));
    }

    #[test]
    fn multi_account_toast_names_the_inbox() {
        let mut x = t(None);
        x.account = "work@example.com".into();
        assert!(toast_text(&x).line2.starts_with("To work@example.com · "));
    }
}
