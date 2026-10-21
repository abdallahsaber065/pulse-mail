//! RFC 822 -> cached message conversion (mail-parser), plus OTP detection.

use super::gmail::{MetaMessage, RawMessage};
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
    m.otp_code = safe_otp(&m.subject, &text);
    m.body_text = truncate(text, MAX_TEXT_BYTES);
    m.body_html = html.filter(|h| h.len() <= MAX_HTML_BYTES);
    m
}

/// Message from `format=metadata`: headers, snippet and labels; no body.
/// Headers go through mail-parser so RFC 2047 encoded words are decoded.
pub fn from_metadata(meta: &MetaMessage) -> NewMessage {
    let mut m = NewMessage {
        id: meta.id.clone(),
        thread_id: meta.thread_id.clone(),
        internal_date: meta.internal_date_ms(),
        snippet: decode_snippet(&meta.snippet),
        label_ids: meta.label_ids.clone(),
        ..Default::default()
    };
    let mut head = String::new();
    for name in ["From", "To", "Subject"] {
        if let Some(v) = meta.header(name) {
            // header values never legitimately contain CR/LF
            let v: String = v
                .chars()
                .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
                .collect();
            head.push_str(&format!("{name}: {v}\r\n"));
        }
    }
    head.push_str("\r\n");
    if let Some(p) = MessageParser::default().parse(head.as_bytes()) {
        m.subject = p.subject().unwrap_or_default().to_string();
        if let Some(addr) = p.from().and_then(Address::first) {
            m.from_name = addr.name().unwrap_or_default().to_string();
            m.from_email = addr.address().unwrap_or_default().to_string();
        }
        if let Some(to) = p.to() {
            m.to_list = to.iter().filter_map(|a| a.address()).collect::<Vec<_>>().join(", ");
        }
    }
    // Most OTP mails put the code in the first line, which is in the snippet.
    m.otp_code = safe_otp(&m.subject, &m.snippet);
    m
}

/// OTP detection must never take sync down, whatever the input.
fn safe_otp(subject: &str, text: &str) -> Option<String> {
    std::panic::catch_unwind(|| otp::extract(subject, text).map(|o| o.code)).unwrap_or_else(|_| {
        tracing::error!("otp extractor panicked; skipping code detection for one message");
        None
    })
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
    fn metadata_decodes_encoded_words_and_finds_otp_in_snippet() {
        let json = r#"{"id":"m","threadId":"t","labelIds":["INBOX","UNREAD"],
            "snippet":"Your verification code is 482913. It expires in 10 minutes.","internalDate":"5",
            "payload":{"headers":[
              {"name":"From","value":"=?UTF-8?B?2LPYp9ix2Kk=?= <sara@example.com>"},
              {"name":"To","value":"me@example.com, Other <o@example.com>"},
              {"name":"Subject","value":"=?utf-8?Q?Verify_=F0=9F=94=90?="}]}}"#;
        let meta: MetaMessage = serde_json::from_str(json).unwrap();
        let m = from_metadata(&meta);
        assert_eq!(m.from_name, "سارة");
        assert_eq!(m.from_email, "sara@example.com");
        assert_eq!(m.subject, "Verify 🔐");
        assert_eq!(m.to_list, "me@example.com, o@example.com");
        assert_eq!(m.otp_code.as_deref(), Some("482913"));
        assert!(m.body_text.is_empty());
    }

    #[test]
    fn metadata_with_plain_utf8_headers() {
        let json = r#"{"id":"m","threadId":"t","snippet":"hi\u00a0there","internalDate":"5",
            "payload":{"headers":[{"name":"From","value":"Facebook <security@facebookmail.com>"},
            {"name":"Subject","value":"Did you just log in near Sidi Bishr?"}]}}"#;
        let m = from_metadata(&serde_json::from_str(json).unwrap());
        assert_eq!(m.from_name, "Facebook");
        assert_eq!(m.subject, "Did you just log in near Sidi Bishr?");
        assert_eq!(m.otp_code, None);
    }

    #[test]
    fn garbage_still_yields_a_row() {
        let m = to_new_message(&raw(&["INBOX"]), b"");
        assert_eq!(m.id, "m1");
    }
}
