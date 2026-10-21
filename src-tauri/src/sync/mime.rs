//! RFC 822 -> cached message conversion (mail-parser), plus OTP detection.

use super::gmail::RawMessage;
use crate::otp;
use crate::store::NewMessage;
use mail_parser::{Address, MessageParser};

/// HTML bodies above this size are dropped from the cache (they are fetched
/// on demand later) to keep the database and memory small.
const MAX_HTML_BYTES: usize = 512 * 1024;
const MAX_TEXT_BYTES: usize = 128 * 1024;

pub fn to_new_message(raw: &RawMessage, bytes: &[u8]) -> NewMessage {
    let parsed = MessageParser::default().parse(bytes);
    let mut m = NewMessage {
        id: raw.id.clone(),
        thread_id: raw.thread_id.clone(),
        internal_date: raw.internal_date_ms(),
        snippet: decode_snippet(&raw.snippet),
        label_ids: raw.label_ids.clone(),
        ..Default::default()
    };
    let Some(p) = parsed else {
        return m;
    };

    m.subject = p.subject().unwrap_or_default().to_string();
    if let Some(addr) = p.from().and_then(Address::first) {
        m.from_name = addr.name().unwrap_or_default().to_string();
        m.from_email = addr.address().unwrap_or_default().to_string();
    }
    if let Some(to) = p.to() {
        m.to_list = to.iter().filter_map(|a| a.address()).collect::<Vec<_>>().join(", ");
    }
    let html = p.body_html(0).map(|s| s.into_owned());
    let text = p
        .body_text(0)
        .map(|s| s.into_owned())
        .or_else(|| html.as_deref().map(otp::html_to_text))
        .unwrap_or_default();
    m.has_attachments = p.attachment_count() > 0;
    m.otp_code = otp::extract(&m.subject, &text).map(|o| o.code);
    m.body_text = truncate(text, MAX_TEXT_BYTES);
    m.body_html = html.filter(|h| h.len() <= MAX_HTML_BYTES);
    m
}

fn truncate(mut s: String, max: usize) -> String {
    if s.len() > max {
        let mut cut = max;
        while !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
    }
    s
}

/// Gmail snippets are HTML-escaped.
fn decode_snippet(s: &str) -> String {
    s.replace("&#39;", "'")
        .replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(labels: &[&str]) -> RawMessage {
        RawMessage {
            id: "m1".into(),
            thread_id: "t1".into(),
            label_ids: labels.iter().map(|s| s.to_string()).collect(),
            snippet: "Your code is 482913 &amp; more".into(),
            internal_date: "1729500000000".into(),
            raw: String::new(),
        }
    }

    #[test]
    fn parses_plain_otp_mail() {
        let eml = b"From: \"GitHub\" <noreply@github.com>\r\nTo: me@example.com\r\nSubject: [GitHub] Please verify your device\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nVerification code: 731590\r\n";
        let m = to_new_message(&raw(&["INBOX", "UNREAD"]), eml);
        assert_eq!(m.from_name, "GitHub");
        assert_eq!(m.from_email, "noreply@github.com");
        assert_eq!(m.to_list, "me@example.com");
        assert_eq!(m.otp_code.as_deref(), Some("731590"));
        assert_eq!(m.snippet, "Your code is 482913 & more");
        assert_eq!(m.internal_date, 1_729_500_000_000);
    }

    #[test]
    fn html_only_mail_gets_text_and_otp() {
        let eml = b"From: Acme <a@acme.test>\r\nSubject: Sign in to Acme\r\nMIME-Version: 1.0\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<html><body><p>Your sign in code is</p><h1>551203</h1></body></html>";
        let m = to_new_message(&raw(&["INBOX"]), eml);
        assert!(m.body_html.as_deref().unwrap_or_default().contains("<h1>551203</h1>"));
        assert!(m.body_text.contains("551203"));
        assert_eq!(m.otp_code.as_deref(), Some("551203"));
    }

    #[test]
    fn garbage_still_yields_a_row() {
        let m = to_new_message(&raw(&["INBOX"]), b"");
        assert_eq!(m.id, "m1");
    }
}
