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
                    (Some("copy"), Some(code)) => crate::copy_to_clipboard(&app, code),
                    _ => crate::window::open_thread(&app, &thread_id, Some(&message_id)),
                }
                Ok(())
            })
            .show();
        if let Err(e) = result {
            tracing::warn!(error = %e, "failed to show toast");
        }
    });
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
        }
    }

    #[test]
    fn otp_toast_leads_with_code() {
        let tt = toast_text(&t(Some("731590")));
        assert!(tt.title.starts_with("731590"));
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
