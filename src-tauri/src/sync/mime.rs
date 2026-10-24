//! RFC 822 -> cached message conversion (mail-parser), plus OTP detection.

use super::gmail::{MetaMessage, RawMessage};
use crate::otp;
use crate::store::{NewAttachment, NewMessage};
use mail_parser::{Address, HeaderValue, MessageParser, MimeHeaders};

/// HTML bodies above this size are dropped from the cache (they are fetched
/// on demand later) to keep the database and memory small.
const MAX_HTML_BYTES: usize = 512 * 1024;
const MAX_TEXT_BYTES: usize = 128 * 1024;
/// Inline (`cid:`) images up to this size are cached so HTML renders offline;
/// other attachments are downloaded on demand.
const MAX_INLINE_IMAGE_BYTES: usize = 1024 * 1024;

pub fn to_new_message(raw: &RawMessage, bytes: &[u8]) -> NewMessage {
    let parsed = MessageParser::default().parse(bytes);
    let mut m = NewMessage {
        id: raw.id.clone(),
        thread_id: raw.thread_id.clone(),
        internal_date: raw.internal_date_ms(),
        snippet: decode_snippet(&raw.snippet),
        label_ids: raw.label_ids.clone(),
        size: if raw.size_estimate > 0 {
            raw.size_estimate
        } else {
            bytes.len() as i64
        },
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
    if let Some(cc) = p.cc() {
        m.cc_list = cc.iter().filter_map(|a| a.address()).collect::<Vec<_>>().join(", ");
    }
    if let Some(rt) = p.reply_to().and_then(Address::first) {
        m.reply_to = rt.address().unwrap_or_default().to_string();
    }
    m.rfc_message_id = p.message_id().map(|id| format!("<{id}>")).unwrap_or_default();
    m.rfc_references = header_ids(p.references());
    m.list_unsubscribe = p
        .headers()
        .iter()
        .find(|h| h.name.as_str().eq_ignore_ascii_case("List-Unsubscribe"))
        .and_then(|h| unsub_value(&h.value))
        .map(|s| sanitize_header_value(&s))
        .filter(|s| !s.is_empty());
    // mail-parser converts text-only mail to HTML; keep real HTML only, so
    // plain text gets our own rendering (linkified, quotes folded).
    let html = p
        .html_part(0)
        .filter(|part| part.is_text_html())
        .and_then(|_| p.body_html(0))
        .map(|s| s.into_owned());
    let text = p
        .body_text(0)
        .map(|s| s.into_owned())
        .or_else(|| html.as_deref().map(otp::html_to_text))
        .unwrap_or_default();
    m.attachments = attachments(&p);
    m.has_attachments = m.attachments.iter().any(|a| !a.inline);
    let (subject, from) = (m.subject.clone(), m.from_email.clone());
    apply_otp(
        &mut m,
        &otp::Input {
            subject: &subject,
            text: &text,
            html: html.as_deref(),
            from: &from,
            promotional: raw.label_ids.iter().any(|l| l == "CATEGORY_PROMOTIONS"),
        },
    );
    m.body_text = truncate(text, MAX_TEXT_BYTES);
    m.body_html = html.filter(|h| h.len() <= MAX_HTML_BYTES);
    m
}

fn header_ids(v: &HeaderValue<'_>) -> String {
    match v {
        HeaderValue::Text(t) => format!("<{t}>"),
        HeaderValue::TextList(l) => l.iter().map(|t| format!("<{t}>")).collect::<Vec<_>>().join(" "),
        _ => String::new(),
    }
}

/// Header values never legitimately contain CR/LF; fold them out before
/// the value is stored or re-parsed.
fn sanitize_header_value(v: &str) -> String {
    v.chars()
        .map(|c| if c == '\r' || c == '\n' { ' ' } else { c })
        .collect()
}

/// mail-parser types List-Unsubscribe as an address list (`<https://…>`
/// looks like an angle-addr); rebuild the angle-bracket URL list either way.
fn unsub_value(v: &HeaderValue<'_>) -> Option<String> {
    match v {
        HeaderValue::Text(t) => Some(t.to_string()),
        HeaderValue::Address(list) => {
            let parts: Vec<String> = list
                .iter()
                .filter_map(|a| a.address().map(|u| format!("<{u}>")))
                .collect();
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        _ => None,
    }
}

/// Attachment list. "Inline" means an image referenced by `cid:` from the
/// HTML body (shown in the message, not in the attachment strip).
fn attachments(p: &mail_parser::Message<'_>) -> Vec<NewAttachment> {
    p.attachments()
        .enumerate()
        .map(|(i, part)| {
            let mime = part
                .content_type()
                .map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or("octet-stream")))
                .unwrap_or_else(|| "application/octet-stream".into())
                .to_ascii_lowercase();
            let bytes = part.contents();
            let content_id = part
                .content_id()
                .map(crate::render::normalize_cid)
                .filter(|c| !c.is_empty());
            let disposition_inline = part.content_disposition().is_none_or(|d| d.is_inline());
            let inline = content_id.is_some() && disposition_inline && mime.starts_with("image/");
            let filename = part
                .attachment_name()
                .map(safe_filename)
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| default_name(&mime, i));
            NewAttachment {
                part: i as u32,
                filename,
                size: bytes.len() as i64,
                data: (inline && bytes.len() <= MAX_INLINE_IMAGE_BYTES && !mime.contains("svg"))
                    .then(|| bytes.to_vec()),
                mime,
                content_id,
                inline,
            }
        })
        .collect()
}

/// Strip path separators, control and reserved characters (Windows), cap length.
pub fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_control() || r#"\/:*?"<>|"#.contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim().to_string();
    let reserved = ["CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "LPT1", "LPT2"];
    let stem = cleaned.split('.').next().unwrap_or_default().to_ascii_uppercase();
    let cleaned = if reserved.contains(&stem.as_str()) {
        format!("_{cleaned}")
    } else {
        cleaned
    };
    let mut out: String = cleaned
        .chars()
        .rev()
        .take(150)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    if out.chars().count() < cleaned.chars().count() {
        // keep the extension, drop the front
        out = format!("…{out}");
    }
    out
}

fn default_name(mime: &str, i: usize) -> String {
    let ext = match mime {
        "message/rfc822" => "eml",
        "application/pdf" => "pdf",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "text/calendar" => "ics",
        "text/plain" => "txt",
        _ => "bin",
    };
    format!("attachment-{}.{ext}", i + 1)
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
        size: meta.size_estimate,
        // Without the MIME tree, multipart/mixed is the usual sign of files.
        has_attachments: meta
            .header("Content-Type")
            .is_some_and(|c| c.trim_start().to_ascii_lowercase().starts_with("multipart/mixed")),
        ..Default::default()
    };
    let mut head = String::new();
    for name in ["From", "To", "Subject"] {
        if let Some(v) = meta.header(name) {
            head.push_str(&format!("{name}: {}\r\n", sanitize_header_value(v)));
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
    m.list_unsubscribe = meta
        .header("List-Unsubscribe")
        .map(sanitize_header_value)
        .filter(|s| !s.is_empty());
    // Most OTP mails put the code in the first line, which is in the snippet.
    let (subject, snippet, from) = (m.subject.clone(), m.snippet.clone(), m.from_email.clone());
    apply_otp(
        &mut m,
        &otp::Input {
            subject: &subject,
            text: &snippet,
            html: None,
            from: &from,
            promotional: meta.label_ids.iter().any(|l| l == "CATEGORY_PROMOTIONS"),
        },
    );
    m
}

/// Run OTP detection and copy the result onto the message.
pub fn apply_otp(m: &mut NewMessage, input: &otp::Input) {
    let d = safe_detect(input);
    m.otp_code = None;
    m.otp_link = None;
    m.otp_score = 0;
    m.otp_expires_min = None;
    if let Some(d) = d {
        m.otp_score = d.confidence;
        m.otp_expires_min = d.expires_min;
        match d.kind {
            otp::Kind::Code => m.otp_code = Some(d.value),
            otp::Kind::Link => m.otp_link = Some(d.value),
        }
    }
}

/// OTP detection must never take sync down, whatever the input.
pub fn safe_detect(input: &otp::Input) -> Option<otp::Detection> {
    std::panic::catch_unwind(|| otp::detect(input)).unwrap_or_else(|_| {
        tracing::error!("otp detector panicked; skipping code detection for one message");
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
            size_estimate: 0,
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
    fn list_unsubscribe_extracted_and_sanitized() {
        // Metadata path (history/batch fetches).
        let json = r#"{"id":"m","threadId":"t","snippet":"deal","internalDate":"5",
            "payload":{"headers":[{"name":"From","value":"Deals <deals@shop.test>"},
            {"name":"List-Unsubscribe","value":"<https://shop.test/u?x=1>, <mailto:leave@shop.test>"}]}}"#;
        let m = from_metadata(&serde_json::from_str(json).unwrap());
        assert_eq!(
            m.list_unsubscribe.as_deref(),
            Some("<https://shop.test/u?x=1>, <mailto:leave@shop.test>")
        );
        // Raw path (full MIME), including a folded header line.
        let eml = b"From: Deals <deals@shop.test>\r\nSubject: deal\r\nList-Unsubscribe: <https://shop.test/leave>,\r\n <mailto:leave@shop.test>\r\nContent-Type: text/plain\r\n\r\nbye\r\n";
        let m = to_new_message(&raw(&["CATEGORY_PROMOTIONS"]), eml);
        let unsub = m.list_unsubscribe.as_deref().unwrap_or_default();
        assert!(
            unsub.contains("https://shop.test/leave") && unsub.contains("mailto:leave@shop.test"),
            "{unsub}"
        );
        // No header -> None on both paths.
        let plain = b"From: a@x.test\r\nSubject: hi\r\n\r\nhello\r\n";
        assert_eq!(to_new_message(&raw(&["INBOX"]), plain).list_unsubscribe, None);
    }

    #[test]
    fn attachments_inline_images_and_threading_headers() {
        let eml = b"From: A <a@x.test>\r\nTo: me@x.test\r\nCc: c@x.test\r\nSubject: Report\r\nMessage-ID: <abc@x.test>\r\nReferences: <r1@x.test> <r2@x.test>\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=b1\r\n\r\n--b1\r\nContent-Type: multipart/related; boundary=b2\r\n\r\n--b2\r\nContent-Type: text/html\r\n\r\n<p>See <img src=\"cid:logo@x\"></p>\r\n--b2\r\nContent-Type: image/png\r\nContent-ID: <Logo@X>\r\nContent-Transfer-Encoding: base64\r\n\r\niVBORw0KGgo=\r\n--b2--\r\n--b1\r\nContent-Type: application/pdf; name=\"../../evil:report?.pdf\"\r\nContent-Disposition: attachment; filename=\"../../evil:report?.pdf\"\r\nContent-Transfer-Encoding: base64\r\n\r\nJVBERi0xLjQK\r\n--b1--\r\n";
        let m = to_new_message(&raw(&["INBOX"]), eml);
        assert_eq!(m.cc_list, "c@x.test");
        assert_eq!(m.rfc_message_id, "<abc@x.test>");
        assert_eq!(m.rfc_references, "<r1@x.test> <r2@x.test>");
        assert_eq!(m.attachments.len(), 2, "{:?}", m.attachments);
        let logo = &m.attachments[0];
        assert!(logo.inline && logo.content_id.as_deref() == Some("logo@x") && logo.data.is_some());
        let pdf = &m.attachments[1];
        assert!(!pdf.inline && pdf.data.is_none());
        assert_eq!(pdf.mime, "application/pdf");
        assert!(
            !pdf.filename.contains('/') && !pdf.filename.contains(':') && pdf.filename.ends_with(".pdf"),
            "{}",
            pdf.filename
        );
        assert!(m.has_attachments);
    }

    #[test]
    fn plain_text_mail_has_no_html() {
        let eml = b"From: a@x.test\r\nSubject: hi\r\nContent-Type: text/plain\r\n\r\nhello https://x.test\r\n";
        let m = to_new_message(&raw(&["INBOX"]), eml);
        assert!(m.body_html.is_none());
        assert!(m.body_text.contains("hello"));
    }

    #[test]
    fn safe_filenames() {
        assert_eq!(safe_filename("CON.txt"), "_CON.txt");
        assert_eq!(safe_filename("a/b\\c.pdf"), "a_b_c.pdf");
        assert!(!safe_filename("..").contains(".."));
    }

    #[test]
    fn garbage_still_yields_a_row() {
        let m = to_new_message(&raw(&["INBOX"]), b"");
        assert_eq!(m.id, "m1");
    }
}
