//! Native Windows toasts. OTP mail gets a "Copy code" button that works
//! without opening the window; clicking the toast opens that exact thread.

use tauri::AppHandle;

#[derive(Debug, Clone)]
pub struct NewMailToast {
    pub message_id: String,
    pub thread_id: String,
    pub from: String,
    pub subject: String,
    pub snippet: String,
    pub otp: Option<String>,
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
    match &t.otp {
        Some(_) if t.hide_code => ToastText {
            title: format!("Verification code from {}", t.from),
            line1: subject,
            line2: "Unlock your PC to see it.".into(),
        },
        Some(code) => ToastText {
            title: format!("{code}  ·  {}", t.from),
            line1: subject,
            line2: "Verification code. Use \"Copy code\" to copy it.".into(),
        },
        None => ToastText {
            title: t.from.clone(),
            line1: subject,
            line2: ellipsize(&t.snippet, 140),
        },
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
pub fn show_new_mail(app: &AppHandle, t: NewMailToast) {
    use tauri_winrt_notification::{Duration, Sound, Toast};

    // Dev builds aren't registered with an AppUserModelID, so borrow
    // PowerShell's (documented limitation). Installed builds use ours.
    let app_id = if cfg!(debug_assertions) {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        app.config().identifier.clone()
    };
    let text = toast_text(&t);
    let app = app.clone();
    // WinRT calls can block briefly; keep them off the async runtime.
    std::thread::spawn(move || {
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
        if t.otp.is_some() {
            toast = toast.add_button("Copy code", "copy").add_button("Open", "open");
        }
        let otp = t.otp.clone();
        let thread_id = t.thread_id.clone();
        let message_id = t.message_id.clone();
        let result = toast
            .on_activated(move |action| {
                match (action.as_deref(), &otp) {
                    (Some("copy"), Some(code)) => crate::copy_secret(&app, code),
                    _ => crate::window::open_thread(&app, &thread_id, Some(&message_id)),
                }
                Ok(())
            })
            .show();
        match result {
            // Timestamped marker for measuring push-to-toast latency (PLAN.md).
            Ok(()) => tracing::info!(otp = t.otp.is_some(), "toast shown"),
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
            message_id: "m".into(),
            thread_id: "t".into(),
            from: "GitHub".into(),
            subject: "Please verify your device".into(),
            snippet: "x".repeat(300),
            otp: otp.map(Into::into),
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
}
