//! Compose: recipients, reply/forward context and the RFC 822 message we
//! hand to Gmail (drafts.create/update, drafts.send). Pure functions; the
//! IPC side lives in `commands::compose`.
//!
//! Everything the user typed is treated as text: headers go through
//! mail-builder (which encodes and folds them, so CR/LF can't inject
//! headers) and the HTML part is built here from escaped text only.

use crate::store::MessageRow;
use mail_builder::MessageBuilder;
use mail_builder::headers::address::Address;
use serde::{Deserialize, Serialize};

pub const MAX_RECIPIENTS: usize = 100;
pub const MAX_BODY: usize = 1_000_000;
pub const MAX_SUBJECT: usize = 500;
/// Gmail's limit for files attached to one message.
pub const MAX_ATTACH_TOTAL: i64 = 25 * 1024 * 1024;
pub const MAX_ATTACHMENTS: usize = 50;
pub const MAX_HTML: usize = 2_000_000;

/// A file in the compose window: either uploaded from this PC (`staged`,
/// held in memory by `commands::compose`) or an attachment of the message
/// being forwarded (`source_message` + `source_part`).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DraftAttachment {
    pub name: String,
    pub mime: String,
    pub size: i64,
    #[serde(default)]
    pub staged: Option<String>,
    #[serde(default)]
    pub source_message: Option<String>,
    #[serde(default)]
    pub source_part: Option<u32>,
}

/// Bytes of one attachment, resolved by the caller.
pub struct FilePart {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}

/// "type/subtype" made of safe characters, else octet-stream.
pub fn safe_mime(m: &str) -> String {
    let m = m.trim().to_ascii_lowercase();
    let ok = m.len() <= 100
        && m.split_once('/').is_some_and(|(a, b)| {
            !a.is_empty()
                && !b.is_empty()
                && (a.to_string() + b)
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"+-.".contains(&c))
        });
    if ok { m } else { "application/octet-stream".into() }
}

/// Sanitize the compose editor's HTML: formatting only (paragraphs, lists,
/// quotes, links, bold/italic/underline/strike, code). Everything else,
/// including styles, classes, images and scripts, is dropped.
pub fn sanitize_compose_html(html: &str) -> String {
    use std::collections::HashSet;
    ammonia::Builder::empty()
        .add_tags([
            "p",
            "br",
            "div",
            "span",
            "strong",
            "b",
            "em",
            "i",
            "u",
            "s",
            "strike",
            "ul",
            "ol",
            "li",
            "blockquote",
            "a",
            "h1",
            "h2",
            "h3",
            "code",
            "pre",
            "hr",
        ])
        .add_generic_attributes(["dir"])
        .add_tag_attributes("a", ["href"])
        .add_tag_attributes("ol", ["start"])
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .url_relative(ammonia::UrlRelative::Deny)
        .link_rel(Some("noopener noreferrer"))
        .strip_comments(true)
        .clean(html)
        .to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Recipient {
    pub name: String,
    pub email: String,
}

/// Split a recipient field ("Ana <ana@x.com>, bob@y.com; …"). Commas inside
/// quoted names are kept. Returns the first invalid entry as the error.
pub fn parse_recipients(field: &str) -> Result<Vec<Recipient>, String> {
    let mut parts = Vec::new();
    let (mut cur, mut quoted, mut angle) = (String::new(), false, false);
    for ch in field.chars() {
        match ch {
            '"' => quoted = !quoted,
            '<' => angle = true,
            '>' => angle = false,
            ',' | ';' | '\n' if !quoted && !angle => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    parts.push(cur);
    let mut out = Vec::new();
    for p in parts.iter().map(|p| p.trim()).filter(|p| !p.is_empty()) {
        let (name, email) = match (p.rfind('<'), p.ends_with('>')) {
            (Some(i), true) => (
                p[..i].trim().trim_matches('"').trim().to_string(),
                p[i + 1..p.len() - 1].trim().to_string(),
            ),
            _ => (String::new(), p.to_string()),
        };
        if !valid_email(&email) || name.chars().any(char::is_control) {
            return Err(p.chars().take(80).collect());
        }
        out.push(Recipient { name, email });
    }
    if out.len() > MAX_RECIPIENTS {
        return Err(format!("more than {MAX_RECIPIENTS} recipients"));
    }
    Ok(out)
}

pub fn valid_email(s: &str) -> bool {
    s.len() <= 254
        && !s
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || "<>,;\"()[]\\".contains(c))
        && s.split_once('@').is_some_and(|(l, d)| {
            !l.is_empty() && !d.contains('@') && d.contains('.') && !d.starts_with('.') && !d.ends_with('.')
        })
}

/// Text quoted below a reply or forward.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Quote {
    /// "On Tue, Oct 21, 2026 at 9:34 PM Ana <ana@x.com> wrote:" or the
    /// forwarded-message header block (one line per header).
    pub header: String,
    pub text: String,
    #[serde(default)]
    pub forward: bool,
}

/// What the compose window sends to save or send.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Draft {
    pub to: String,
    #[serde(default)]
    pub cc: String,
    #[serde(default)]
    pub bcc: String,
    pub subject: String,
    pub body: String,
    #[serde(default)]
    pub quote: Option<Quote>,
    /// `<id@host>` of the message being answered.
    #[serde(default)]
    pub in_reply_to: String,
    #[serde(default)]
    pub references: String,
    #[serde(default)]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub draft_id: Option<String>,
    /// Rich body from the editor (sanitized again in Rust). `body` is its
    /// plain-text version and is used for the text/plain part.
    #[serde(default)]
    pub html: Option<String>,
    #[serde(default)]
    pub attachments: Vec<DraftAttachment>,
}

#[derive(Debug)]
pub struct Prepared {
    pub to: Vec<Recipient>,
    pub cc: Vec<Recipient>,
    pub bcc: Vec<Recipient>,
}

impl Draft {
    /// Validate the fields. `sending` requires at least one recipient.
    pub fn prepare(&self, sending: bool) -> Result<Prepared, String> {
        let field = |name: &str, v: &str| {
            parse_recipients(v).map_err(|bad| format!("{name}: \"{bad}\" isn't a valid address."))
        };
        let p = Prepared {
            to: field("To", &self.to)?,
            cc: field("Cc", &self.cc)?,
            bcc: field("Bcc", &self.bcc)?,
        };
        if sending && p.to.is_empty() && p.cc.is_empty() && p.bcc.is_empty() {
            return Err("Add at least one recipient.".into());
        }
        if p.to.len() + p.cc.len() + p.bcc.len() > MAX_RECIPIENTS {
            return Err(format!("Up to {MAX_RECIPIENTS} recipients per message."));
        }
        let quote_len = self.quote.as_ref().map_or(0, |q| q.text.len() + q.header.len());
        if self.body.len() + quote_len > MAX_BODY || self.subject.len() > MAX_SUBJECT || self.body.contains('\0') {
            return Err("This message is too long.".into());
        }
        if self.html.as_ref().is_some_and(|h| h.len() > MAX_HTML) {
            return Err("This message is too long.".into());
        }
        if self.attachments.len() > MAX_ATTACHMENTS {
            return Err(format!("Up to {MAX_ATTACHMENTS} attachments per message."));
        }
        let total: i64 = self.attachments.iter().map(|a| a.size.max(0)).sum();
        if total > MAX_ATTACH_TOTAL {
            return Err("Attachments are larger than Gmail's 25 MB limit.".into());
        }
        for a in &self.attachments {
            if a.name.is_empty() || a.name.len() > 255 || (a.staged.is_none() && a.source_message.is_none()) {
                return Err("One of the attachments isn't available.".into());
            }
        }
        Ok(p)
    }
}

/// `<a@b> <c@d>` → ["a@b", "c@d"]; anything that isn't a bracketed id is dropped.
pub fn message_ids(s: &str) -> Vec<String> {
    s.split_whitespace()
        .filter_map(|t| t.strip_prefix('<')?.strip_suffix('>'))
        .filter(|id| {
            !id.is_empty()
                && id.len() <= 250
                && id.contains('@')
                && !id.chars().any(|c| c.is_control() || "<> ".contains(c))
        })
        .map(str::to_owned)
        .collect()
}

fn single_line(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn escape_html(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 16);
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            '\r' => {}
            '\n' => o.push_str("<br>\n"),
            c => o.push(c),
        }
    }
    o
}

/// Plain-text part: body, then the quote (`> ` prefixed for replies).
pub fn text_body(d: &Draft) -> String {
    let mut out = d.body.replace("\r\n", "\n");
    if let Some(q) = &d.quote {
        out.push_str("\n\n");
        out.push_str(&q.header);
        out.push('\n');
        if q.forward {
            out.push('\n');
            out.push_str(&q.text);
        } else {
            for line in q.text.lines() {
                out.push_str(if line.is_empty() { ">" } else { "> " });
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

/// HTML part: the sanitized editor HTML (or escaped text), then the quote
/// built from escaped text (Gmail-compatible quote markup).
pub fn html_body(d: &Draft) -> String {
    let mut out = match d.html.as_deref().filter(|h| !h.trim().is_empty()) {
        Some(h) => format!("<div dir=\"auto\">{}</div>", sanitize_compose_html(h)),
        None => format!("<div dir=\"auto\">{}</div>", escape_html(&d.body)),
    };
    if let Some(q) = &d.quote {
        if q.forward {
            out.push_str(&format!(
                "<br><div class=\"gmail_quote\"><div dir=\"auto\" class=\"gmail_attr\">{}<br></div><br><div dir=\"auto\">{}</div></div>",
                escape_html(&q.header),
                escape_html(&q.text)
            ));
        } else {
            out.push_str(&format!(
                "<br><div class=\"gmail_quote\"><div dir=\"auto\" class=\"gmail_attr\">{}<br></div>\
                 <blockquote class=\"gmail_quote\" style=\"margin:0 0 0 .8ex;border-left:1px #ccc solid;padding-left:1ex\">\
                 <div dir=\"auto\">{}</div></blockquote></div>",
                escape_html(&q.header),
                escape_html(&q.text)
            ));
        }
    }
    out
}

fn addresses(list: &[Recipient]) -> Address<'static> {
    Address::new_list(
        list.iter()
            .map(|r| {
                let name = (!r.name.is_empty()).then(|| single_line(&r.name));
                Address::new_address(name, r.email.clone())
            })
            .collect(),
    )
}

/// RFC 822 bytes for Gmail. `from` is the account address. Bcc is kept in
/// the message (Gmail strips it when sending and needs it to deliver).
pub fn build_raw(d: &Draft, p: &Prepared, from: &str, from_name: &str, files: Vec<FilePart>) -> Vec<u8> {
    let name = single_line(from_name);
    let mut b = MessageBuilder::new().from(Address::new_address(
        (!name.is_empty()).then_some(name),
        from.to_string(),
    ));
    if !p.to.is_empty() {
        b = b.to(addresses(&p.to));
    }
    if !p.cc.is_empty() {
        b = b.cc(addresses(&p.cc));
    }
    if !p.bcc.is_empty() {
        b = b.bcc(addresses(&p.bcc));
    }
    b = b.subject(single_line(&d.subject));
    if let Some(id) = message_ids(&d.in_reply_to).into_iter().next() {
        b = b.in_reply_to(id);
    }
    let refs = message_ids(&d.references);
    if !refs.is_empty() {
        b = b.references(refs);
    }
    b = b.header("X-Mailer", mail_builder::headers::raw::Raw::new("Pulse Mail"));
    b = b.text_body(text_body(d)).html_body(html_body(d));
    for f in files {
        let name = crate::sync::mime::safe_filename(&single_line(&f.name));
        b = b.attachment(
            safe_mime(&f.mime),
            if name.is_empty() {
                "attachment".to_string()
            } else {
                name
            },
            f.bytes,
        );
    }
    b.write_to_vec().unwrap_or_default()
}

/// Reply / reply all / forward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Reply,
    ReplyAll,
    Forward,
}

/// Prefilled fields for answering `m` from the account `me`.
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ComposeInit {
    pub to: String,
    pub cc: String,
    pub subject: String,
    pub in_reply_to: String,
    pub references: String,
    pub thread_id: Option<String>,
    /// Original message, for the quote header and text (built in the UI so
    /// dates use the user's locale).
    pub original_from: String,
    pub original_to: String,
    pub original_cc: String,
    pub original_date: i64,
    pub original_subject: String,
    pub original_text: String,
    /// Forward: the original's files, attached by default.
    pub attachments: Vec<DraftAttachment>,
}

fn prefixed(subject: &str, prefix: &str) -> String {
    let s = subject.trim();
    let lower = s.to_lowercase();
    let has = match prefix {
        "Re:" => lower.starts_with("re:") || lower.starts_with("aw:"),
        _ => lower.starts_with("fwd:") || lower.starts_with("fw:"),
    };
    if has {
        s.to_string()
    } else if s.is_empty() {
        prefix.to_string()
    } else {
        format!("{prefix} {s}")
    }
}

fn display(name: &str, email: &str) -> String {
    if name.is_empty() || name == email {
        email.to_string()
    } else {
        format!("\"{}\" <{email}>", name.replace('"', ""))
    }
}

pub fn reply_context(m: &MessageRow, me: &str, mode: Mode) -> ComposeInit {
    let me_l = me.to_lowercase();
    let not_me = |e: &&str| !e.trim().is_empty() && e.trim().to_lowercase() != me_l;
    let split = |s: &str| {
        s.split(',')
            .map(str::trim)
            .filter(|e| !e.is_empty())
            .map(str::to_owned)
            .collect::<Vec<_>>()
    };
    let sender = if m.reply_to.trim().is_empty() {
        m.from_email.clone()
    } else {
        m.reply_to.clone()
    };
    let from_me = m.from_email.eq_ignore_ascii_case(me);

    let mut init = ComposeInit {
        original_from: display(&m.from_name, &m.from_email),
        original_to: m.to_list.clone(),
        original_cc: m.cc_list.clone(),
        original_date: m.internal_date,
        original_subject: m.subject.clone(),
        original_text: m.body_text.clone(),
        ..Default::default()
    };
    match mode {
        Mode::Forward => {
            init.subject = prefixed(&m.subject, "Fwd:");
            init.attachments = m
                .attachments
                .iter()
                .filter(|a| !a.inline)
                .map(|a| DraftAttachment {
                    name: a.filename.clone(),
                    mime: a.mime.clone(),
                    size: a.size,
                    staged: None,
                    source_message: Some(m.id.clone()),
                    source_part: Some(a.part),
                })
                .collect();
        }
        Mode::Reply | Mode::ReplyAll => {
            init.subject = prefixed(&m.subject, "Re:");
            init.thread_id = Some(m.thread_id.clone());
            init.in_reply_to = m.rfc_message_id.clone();
            init.references = format!("{} {}", m.rfc_references, m.rfc_message_id).trim().to_string();
            // Answering your own message goes back to its recipients (Gmail).
            let mut to: Vec<String> = if from_me {
                split(&m.to_list)
            } else if sender.eq_ignore_ascii_case(&m.from_email) {
                vec![display(&m.from_name, &m.from_email)]
            } else {
                vec![sender.clone()]
            };
            let mut cc: Vec<String> = Vec::new();
            if mode == Mode::ReplyAll {
                if !from_me {
                    to.extend(split(&m.to_list).into_iter().filter(|e| not_me(&e.as_str())));
                }
                cc = split(&m.cc_list).into_iter().filter(|e| not_me(&e.as_str())).collect();
            }
            let mut seen = std::collections::HashSet::new();
            let key = |s: &str| {
                s.rsplit('<')
                    .next()
                    .unwrap_or(s)
                    .trim_end_matches('>')
                    .trim()
                    .to_lowercase()
            };
            to.retain(|e| seen.insert(key(e)));
            cc.retain(|e| seen.insert(key(e)));
            init.to = to.join(", ");
            init.cc = cc.join(", ");
        }
    }
    init
}

#[cfg(test)]
mod tests;
