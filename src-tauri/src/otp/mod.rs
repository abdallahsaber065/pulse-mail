//! One-time-code (OTP / verification code) and magic-link detection.
//!
//! A scored pipeline (see ADR 0022), pure and synchronous so it can run on
//! every incoming message:
//! 1. Normalize: Arabic-Indic / full-width digits to ASCII, zero-width
//!    characters dropped, exotic spaces to plain spaces.
//! 2. WebOTP origin-bound line (`@example.com #123456`) wins outright.
//! 3. Gate: the mail must name a code (multilingual), or say "verify/sign in"
//!    and visibly emphasize a token in its HTML.
//! 4. Score every 4-8 character candidate (digits, `123 456`, `123-456`,
//!    `G-123456`, mixed uppercase alphanumerics) on shape, keyword proximity,
//!    layout (own line, after a colon, subject line, bold/big/monospace HTML),
//!    sender and expiry wording; subtract for dates, times, phones, money,
//!    URLs, e-mail addresses, years and order/tracking/promo context. Weights
//!    are tuned against the labeled corpus in `corpus.rs`.
//! 5. No code: look for a one-click sign-in / verification link.
//!
//! A wrong "Copy code" button is worse than none, so the thresholds are high.

mod html;
mod lexicon;

#[cfg(test)]
mod corpus;

use lexicon::*;
use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

/// Only the first part of a body is scanned; codes live near the top.
const MAX_SCAN_CHARS: usize = 6000;
/// HTML scanned for emphasis and links.
const MAX_HTML_SCAN: usize = 300 * 1024;
/// Lowest confidence worth keeping (a code chip in the list and reader).
pub const STORE_THRESHOLD: i32 = 50;
/// Lowest confidence that may show the code in a toast.
pub const TOAST_THRESHOLD: i32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Code,
    Link,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Detection {
    pub kind: Kind,
    /// Normalized code (separators removed) or the link URL.
    pub value: String,
    /// 0-100.
    pub confidence: i32,
    /// "Expires in N minutes", when the mail says so.
    pub expires_min: Option<u32>,
}

/// What the detector looks at. `html` and `from` are optional extras.
#[derive(Debug, Clone, Copy, Default)]
pub struct Input<'a> {
    pub subject: &'a str,
    pub text: &'a str,
    pub html: Option<&'a str>,
    pub from: &'a str,
    /// Gmail put it in Promotions.
    pub promotional: bool,
}

/// Result of the code-only API (tests).
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OtpMatch {
    pub code: String,
    pub score: i32,
}

/// Code-only detection from subject and plain text.
#[cfg(test)]
pub fn extract(subject: &str, body: &str) -> Option<OtpMatch> {
    let d = detect(&Input { subject, text: body, ..Default::default() })?;
    (d.kind == Kind::Code && d.confidence >= STORE_THRESHOLD).then_some(OtpMatch { code: d.value, score: d.confidence })
}

/// Best code or link at or above [`STORE_THRESHOLD`], if any.
pub fn detect(input: &Input) -> Option<Detection> {
    let subject = normalize(input.subject);
    let body: String = normalize(input.text).chars().take(MAX_SCAN_CHARS).collect();
    let text = format!("{subject}\n{body}");
    let expires_min = expiry_minutes(&text);

    if let Some(c) = WEBOTP.captures_iter(&text).last() {
        return Some(Detection { kind: Kind::Code, value: c[2].to_string(), confidence: 95, expires_min });
    }

    let html_head = input.html.map(|h| head(h, MAX_HTML_SCAN));
    let emphasized = html_head.map(html::emphasized).unwrap_or_default();
    let strong: Vec<(usize, usize)> = STRONG.find_iter(&text).map(|m| (m.start(), m.end())).collect();
    let weak: Vec<(usize, usize)> = WEAK.find_iter(&text).map(|m| (m.start(), m.end())).collect();
    let promo: Vec<(usize, usize)> = PROMO.find_iter(&text).map(|m| (m.start(), m.end())).collect();
    let negative: Vec<(usize, usize)> = NEGATIVE.find_iter(&text).map(|m| (m.start(), m.end())).collect();
    let blocked: Vec<(usize, usize)> = NOT_CODE_SPANS.find_iter(&text).map(|m| (m.start(), m.end())).collect();

    let gated = !strong.is_empty() || (!weak.is_empty() && !emphasized.is_empty());
    let code = if gated {
        best_code(&Ctx {
            text: &text,
            subject_end: subject.len(),
            strong: &strong,
            weak: &weak,
            negative: &negative,
            promo: &promo,
            blocked: &blocked,
            emphasized: &emphasized,
            security_sender: SECURITY_SENDER.is_match(input.from),
            promotional: input.promotional,
            expiry: expires_min.is_some(),
        })
    } else {
        None
    };
    if let Some((confidence, value)) = code.filter(|(s, _)| *s >= STORE_THRESHOLD) {
        return Some(Detection { kind: Kind::Code, value, confidence, expires_min });
    }
    if input.promotional {
        return None;
    }
    best_link(html_head, &text, &weak, &strong)
        .filter(|(s, _)| *s >= STORE_THRESHOLD)
        .map(|(confidence, value)| Detection { kind: Kind::Link, value, confidence, expires_min })
}

struct Ctx<'a> {
    text: &'a str,
    subject_end: usize,
    strong: &'a [(usize, usize)],
    weak: &'a [(usize, usize)],
    negative: &'a [(usize, usize)],
    promo: &'a [(usize, usize)],
    blocked: &'a [(usize, usize)],
    emphasized: &'a std::collections::HashSet<String>,
    security_sender: bool,
    promotional: bool,
    expiry: bool,
}

fn best_code(c: &Ctx) -> Option<(i32, String)> {
    let text = c.text;
    let mut best: Option<(i32, String)> = None;
    for caps in CANDIDATE.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        let raw = caps.get(1).unwrap().as_str();
        let (start, end) = (whole.start(), whole.end());
        let code: String = raw.chars().filter(char::is_ascii_alphanumeric).collect();
        let digits_only = code.bytes().all(|b| b.is_ascii_digit());
        let has_sep = raw.len() != code.len();

        if c.blocked.iter().any(|&(s, e)| s < end && start < e) || footer_line(text, start, end) {
            continue;
        }
        let mut score = if digits_only {
            match (code.len(), has_sep) {
                (6, false) => 30,
                (6, true) => 28,
                (8, true) => 22,
                (5, _) => 20,
                (4, _) => 16,
                (7 | 8, _) => 18,
                _ => continue,
            }
        } else {
            let has_digit = code.bytes().any(|b| b.is_ascii_digit());
            let has_alpha = code.bytes().any(|b| b.is_ascii_alphabetic());
            if !(has_digit && has_alpha) || code.len() < 5 {
                continue;
            }
            if code.len() == 6 { 20 } else { 14 }
        };

        // Keyword proximity, in characters.
        let d_strong_before = nearest_before(text, c.strong, start);
        let d_strong_after = nearest_after(text, c.strong, end);
        let d_weak_before = nearest_before(text, c.weak, start);
        let mut kw = 0;
        // Short numbers need the keyword closer.
        let near = if digits_only && code.len() <= 5 { 40 } else { 80 };
        match d_strong_before {
            Some(d) if d <= near => kw = 40,
            Some(d) if d <= 250 => kw = 22,
            _ => {}
        }
        if d_strong_after.is_some_and(|d| d <= 40) {
            kw = kw.max(28);
        }
        if kw == 0 && d_weak_before.is_some_and(|d| d <= 60) {
            kw = 15;
        }
        if kw == 0 && !digits_only {
            continue; // mixed tokens need a keyword
        }
        score += kw;

        // Layout.
        let prev_char = text[..start].trim_end_matches([' ', '\t']).chars().last();
        let colon = matches!(prev_char, Some(':' | '：' | '-' | '–'));
        let alone = own_line(text, start, end);
        let emph = c.emphasized.contains(&code);
        let direct = strong_end_before(c.strong, start).is_some_and(|kw_end| filler_only(&text[kw_end..start]));
        // A short number loosely near "code" ("code with 4821 examples") is weak evidence.
        if digits_only && code.len() <= 5 && kw == 40 && !(colon || alone || emph || direct) {
            score -= 25;
        }
        if direct {
            score += 5;
        }
        if colon {
            score += 10;
        }
        if alone {
            score += 15;
        }
        if start < c.subject_end {
            score += 15;
        }
        if emph {
            score += 25;
        }
        if c.security_sender {
            score += 5;
        }
        if c.expiry {
            score += 5;
        }

        // Penalties.
        if digits_only && code.len() == 4 && YEAR.is_match(&code) {
            score -= 35;
        }
        // Order/tracking/phone words that label the number, or the keyword
        // itself ("promo code", "postal code", "booking confirmation code").
        let d_neg_before = nearest_before(text, c.negative, start).filter(|d| *d <= 25);
        let line_end = text[end..].find('\n').map_or(text.len(), |i| end + i);
        let d_neg_after = nearest_after(&text[..line_end], c.negative, end).filter(|d| *d <= 20);
        let neg_labels = d_neg_before.is_some_and(|n| d_strong_before.is_none_or(|s| n < s));
        let neg_follows = d_neg_after
            .is_some_and(|n| d_strong_before.is_none_or(|s| s > 30) && d_strong_after.is_none_or(|s| n < s));
        let neg_names_keyword = d_strong_before.is_some_and(|d| d <= 80)
            && strong_start_before(c.strong, start)
                .and_then(|kw| nearest_before(text, c.negative, kw))
                .is_some_and(|d| d <= 25);
        if neg_labels || neg_follows || neg_names_keyword {
            score -= 60;
        }
        let promo_near = nearest_before(text, c.promo, start).is_some_and(|d| d <= 40)
            || nearest_after(text, c.promo, end).is_some_and(|d| d <= 40);
        if promo_near {
            score -= 60;
        }
        if numeric_context(text, start, end) {
            score -= 50;
        }
        if in_url_or_email(text, start, end) {
            score -= 60;
        }
        if c.promotional {
            score -= 35;
        }

        let score = score.clamp(0, 100);
        #[cfg(test)]
        if std::env::var_os("OTP_DEBUG").is_some() {
            eprintln!("  cand {code:>9} score {score}");
        }
        if best.as_ref().is_none_or(|(s, _)| score > *s) {
            best = Some((score, code));
        }
    }
    best
}

fn best_link(html: Option<&str>, text: &str, weak: &[(usize, usize)], strong: &[(usize, usize)]) -> Option<(i32, String)> {
    static URL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"https?://[^\s<>"')\]]+"#).unwrap());
    let mut links = html.map(html::links).unwrap_or_default();
    if links.is_empty() {
        links = URL.find_iter(text).map(|m| (m.as_str().to_string(), String::new())).collect();
    }
    let mail_talks = LINK_TEXT.is_match(text) || !weak.is_empty();
    let mut best: Option<(i32, String)> = None;
    for (url, anchor) in links {
        if url.len() > 2000 || LINK_EXCLUDE.is_match(&url) || LINK_EXCLUDE.is_match(&anchor) {
            continue;
        }
        let mut score = 0;
        if LINK_URL_HINT.is_match(&url) {
            score += 30;
        }
        if has_long_token(&url) {
            score += 15;
        }
        if LINK_TEXT.is_match(&anchor) {
            score += 25;
        }
        if mail_talks {
            score += 10;
        }
        if !strong.is_empty() {
            score -= 10; // talks about a code we couldn't find: don't guess a link
        }
        if best.as_ref().is_none_or(|(s, _)| score > *s) {
            best = Some((score, url));
        }
    }
    best
}

/// First `max` bytes of `s`, cut at a char boundary.
fn head(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut i = max;
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    &s[..i]
}

fn has_long_token(url: &str) -> bool {
    url.split(['/', '?', '&', '=', '#'])
        .any(|p| p.len() >= 16 && p.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.'))
        && url.matches('.').count() < 12
}

fn expiry_minutes(text: &str) -> Option<u32> {
    let c = EXPIRY.captures(text)?;
    let (n, unit) = match (c.get(1), c.get(2), c.get(3), c.get(4)) {
        (Some(n), Some(u), _, _) | (_, _, Some(n), Some(u)) => (n.as_str(), u.as_str().to_lowercase()),
        _ => return None,
    };
    let n: u32 = n.parse().ok()?;
    let mins = if unit.starts_with('h') || unit.starts_with("st") || unit.starts_with("ساع") {
        n * 60
    } else if unit.starts_with("sec") || unit.starts_with("ثان") {
        n.div_ceil(60)
    } else {
        n
    };
    (1..=48 * 60).contains(&mins).then_some(mins)
}

/// Map Arabic-Indic, Eastern Arabic-Indic and full-width digits to ASCII,
/// drop zero-width characters and turn unusual spaces into plain spaces.
pub(crate) fn normalize(s: &str) -> String {
    s.chars()
        .filter_map(|c| match c {
            '\u{0660}'..='\u{0669}' => Some(char::from(b'0' + (c as u32 - 0x0660) as u8)),
            '\u{06F0}'..='\u{06F9}' => Some(char::from(b'0' + (c as u32 - 0x06F0) as u8)),
            '\u{FF10}'..='\u{FF19}' => Some(char::from(b'0' + (c as u32 - 0xFF10) as u8)),
            '\u{200B}'..='\u{200F}' | '\u{2060}' | '\u{FEFF}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' => None,
            '\u{00A0}' | '\u{2000}'..='\u{200A}' | '\u{202F}' | '\u{205F}' | '\u{3000}' => Some(' '),
            _ => Some(c),
        })
        .collect()
}

/// Characters between the nearest keyword ending before `idx` and `idx`.
fn nearest_before(text: &str, spans: &[(usize, usize)], idx: usize) -> Option<usize> {
    spans
        .iter()
        .filter(|&&(_, e)| e <= idx)
        .map(|&(_, e)| text[e..idx].chars().count())
        .min()
}

fn nearest_after(text: &str, spans: &[(usize, usize)], idx: usize) -> Option<usize> {
    spans
        .iter()
        .filter(|&&(s, _)| s >= idx && s <= text.len())
        .map(|&(s, _)| text[idx..s].chars().count())
        .min()
}

/// Start of the closest strong keyword ending before `idx`.
fn strong_start_before(spans: &[(usize, usize)], idx: usize) -> Option<usize> {
    spans.iter().filter(|&&(_, e)| e <= idx).max_by_key(|&&(_, e)| e).map(|&(s, _)| s)
}

fn strong_end_before(spans: &[(usize, usize)], idx: usize) -> Option<usize> {
    spans.iter().filter(|&&(_, e)| e <= idx).map(|&(_, e)| e).max()
}

/// Only linking words between the keyword and the code ("code is", "رمز التحقق الخاص بك هو").
fn filler_only(between: &str) -> bool {
    const FILLER: &[&str] = &[
        "is", "was", "your", "the", "below", "here", "it", "lautet", "ist", "est", "es", "de", "for", "to", "use",
        "verification", "security", "login", "sign-in", "one-time", "vérification", "verificación",
        "هو", "هي", "الخاص", "الخاصة", "بك", "التفعيل", "التحقق", "الدخول",
    ];
    let words: Vec<&str> = between
        .split(|c: char| c.is_whitespace() || matches!(c, ':' | '：' | ',' | '،' | '[' | '(' | '"' | '«'))
        .filter(|w| !w.is_empty())
        .collect();
    words.len() <= 6 && words.iter().all(|w| FILLER.contains(&w.to_lowercase().as_str()))
}

/// Copyright / address / unsubscribe footer lines.
fn footer_line(text: &str, start: usize, end: usize) -> bool {
    let ls = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let le = text[end..].find('\n').map_or(text.len(), |i| end + i);
    let line = text[ls..le].to_lowercase();
    line.contains('©') || line.contains("(c)") || line.contains("all rights reserved") || line.contains("unsubscribe")
}

fn own_line(text: &str, start: usize, end: usize) -> bool {
    let ls = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let le = text[end..].find('\n').map_or(text.len(), |i| end + i);
    let before = text[ls..start].trim();
    let after = text[end..le].trim().trim_end_matches(['.', '!']);
    (before.is_empty() || before.ends_with(':')) && after.is_empty()
}

/// Decimals, money, percentages, times, `#` refs, and digit groups.
fn numeric_context(text: &str, start: usize, end: usize) -> bool {
    let prev: Vec<char> = text[..start].chars().rev().take(6).collect();
    let next: Vec<char> = text[end..].chars().take(3).collect();
    let p0 = prev.first().copied();
    let p1 = prev.get(1).copied();
    let n0 = next.first().copied();
    let n1 = next.get(1).copied();
    let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
    if matches!(n0, Some('.' | ',' | ':')) && digit(n1) {
        return true;
    }
    if matches!(p0, Some('.' | ',' | ':')) && digit(p1) {
        return true;
    }
    if matches!(n0, Some('%')) || (n0 == Some(' ') && n1 == Some('%')) {
        return true;
    }
    if prev.iter().take(2).any(|c| matches!(c, '$' | '€' | '£' | '¥' | '₹' | '+' | '#' | '№')) {
        return true;
    }
    if (p0 == Some(' ') && digit(p1)) || (n0 == Some(' ') && digit(n1)) {
        return true;
    }
    const UNITS: &[&str] = &[
        "egp", "usd", "eur", "sar", "aed", "gbp", "kwd", "le", "ج.م", "جنيه", "ريال", "درهم", "mb", "gb", "kb", "px",
        "pts", "points", "miles", "km", "kg",
    ];
    let clean = |w: &str| w.trim_matches(|c: char| !c.is_alphanumeric() && c != '.').to_lowercase();
    let prev_word = text[..start].split_whitespace().last().map(clean).unwrap_or_default();
    let next_word = text[end..].split_whitespace().next().map(clean).unwrap_or_default();
    UNITS.contains(&prev_word.as_str()) || UNITS.contains(&next_word.as_str())
}

fn in_url_or_email(text: &str, start: usize, end: usize) -> bool {
    let tok_start = text[..start]
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);
    let tok_end = text[end..].find(char::is_whitespace).map(|i| end + i).unwrap_or(text.len());
    let tok = &text[tok_start..tok_end];
    tok.contains("://") || tok.contains("www.") || tok.contains('=') || tok.contains('/') || tok.contains('@')
}

/// Very small HTML-to-text conversion, good enough for OTP scanning and
/// search indexing. Drops `<style>`/`<script>`/`<head>`, turns block tags into
/// newlines and decodes the common entities.
pub fn html_to_text(html: &str) -> String {
    static DROP: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)<(?:style|script|head|title)\b.*?</(?:style|script|head|title)\s*>|<!--.*?-->").unwrap()
    });
    static BLOCK: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?i)<(?:br|/p|/div|/tr|/li|/h[1-6]|/td|/table)\b[^>]*>").unwrap());
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());
    static NUMERIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"&#(x?)([0-9a-fA-F]+);").unwrap());
    static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[ \t\u{00A0}]+").unwrap());
    static NEWLINES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n\s*\n+").unwrap());

    let s = DROP.replace_all(html, " ");
    let s = BLOCK.replace_all(&s, "\n");
    let s = TAG.replace_all(&s, " ");
    let s = NUMERIC.replace_all(&s, |c: &regex::Captures| {
        let radix = if c[1].is_empty() { 10 } else { 16 };
        u32::from_str_radix(&c[2], radix)
            .ok()
            .and_then(char::from_u32)
            .map(String::from)
            .unwrap_or_default()
    });
    let s = s
        .replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&amp;", "&");
    let s = SPACES.replace_all(&s, " ");
    let s = NEWLINES.replace_all(&s, "\n\n");
    s.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(subject: &str, body: &str) -> Option<String> {
        extract(subject, body).map(|m| m.code)
    }

    #[test]
    fn google_prefixed_code() {
        assert_eq!(
            code(
                "G-582913 is your Google verification code",
                "G-582913 is your Google verification code."
            ),
            Some("582913".into())
        );
    }

    #[test]
    fn plain_verification_code() {
        assert_eq!(
            code(
                "Verify your email",
                "Hi,\n\nYour verification code is 482913. It expires in 10 minutes."
            ),
            Some("482913".into())
        );
    }

    #[test]
    fn github_device_code() {
        let body =
            "Hey there!\n\nVerification code: 731590\n\nIf you did not attempt to sign in, change your password.";
        assert_eq!(code("[GitHub] Please verify your device", body), Some("731590".into()));
    }

    #[test]
    fn microsoft_four_digit_security_code() {
        assert_eq!(
            code(
                "Microsoft account security code",
                "Please use the following security code for the Microsoft account.\n\nSecurity code: 7391"
            ),
            Some("7391".into())
        );
    }

    #[test]
    fn spaced_code_is_normalized() {
        assert_eq!(
            code("Your login code", "Your login code is 123 456"),
            Some("123456".into())
        );
    }

    #[test]
    fn hyphenated_code_is_normalized() {
        assert_eq!(
            code("Sign in", "Enter this code to sign in: 834-221"),
            Some("834221".into())
        );
    }

    #[test]
    fn alphanumeric_code() {
        assert_eq!(
            code("Your one-time passcode", "Your one-time passcode: A7K9QZ"),
            Some("A7K9QZ".into())
        );
    }

    #[test]
    fn arabic_with_arabic_indic_digits() {
        assert_eq!(
            code("رمز التحقق", "رمز التحقق الخاص بك هو ٤٥٨٢١٣"),
            Some("458213".into())
        );
    }

    #[test]
    fn prefers_code_over_year_and_amounts() {
        let body = "Your verification code is 482913. It expires on 21 Oct 2026. Total charged: $12.50";
        assert_eq!(code("Verify", body), Some("482913".into()));
    }

    #[test]
    fn code_after_number_phrase() {
        assert_eq!(
            code("Welcome", "550912 is your Instagram code. Don't share it."),
            Some("550912".into())
        );
    }

    #[test]
    fn ignores_order_numbers_without_otp_wording() {
        assert_eq!(
            code(
                "Your order has shipped",
                "Order #48213 has shipped. Total: $49.99. Delivery by Oct 2026."
            ),
            None
        );
    }

    #[test]
    fn ignores_promo_codes() {
        assert_eq!(code("Big sale", "Use promo code SAVE20 for 20% off everything"), None);
    }

    #[test]
    fn ignores_codes_inside_urls() {
        let body = "Verify your account: https://example.com/verify?token=482913 \nThanks";
        assert_eq!(code("Verify your account", body), None);
    }

    #[test]
    fn ignores_plain_uppercase_words() {
        assert_eq!(
            code("Code of conduct", "Please read our CODE OF CONDUCT and HAPPY coding"),
            None
        );
    }

    #[test]
    fn ignores_newsletters() {
        let body = "This week in Rust 612. 25 new crates, 1500 contributors. Read more at the blog.";
        assert_eq!(code("This Week in Rust", body), None);
    }

    #[test]
    fn nbsp_before_code_does_not_panic() {
        // Regression: in_url_token sliced inside a 2-byte NBSP.
        let body = "Your verification code is\u{a0}482913\u{a0}and expires soon";
        assert_eq!(code("Verify", body), Some("482913".into()));
    }

    #[test]
    fn emoji_and_wide_spaces_around_code() {
        let body = "🔐 Your login code:\u{2003}731590 ✅\u{3000}thanks 🙏";
        assert_eq!(code("Sign in 🔑", body), Some("731590".into()));
    }

    #[test]
    fn arabic_text_with_nbsp_and_ascii_digits() {
        let body = "مرحبا،\u{a0}رمز\u{a0}التحقق:\u{a0}558812\u{a0}صالح لمدة ١٠ دقائق";
        assert_eq!(code("رمز التحقق", body), Some("558812".into()));
    }

    /// Insert awkward multi-byte characters at every position around a code
    /// and make sure nothing panics (char-boundary safety for all slicing).
    #[test]
    fn never_panics_on_multibyte_text() {
        let tricky = [
            '\u{a0}', '\u{2003}', '\u{3000}', '\u{200b}', 'é', 'ر', '٤', '🔐', '\u{200d}', '\u{feff}',
        ];
        let base = "Your code is 482913. Visit https://x.y/a?b=1 or call +1 800 555 0199. Total $12.50 in 2026.";
        let chars: Vec<char> = base.chars().collect();
        for t in tricky {
            for i in 0..=chars.len() {
                let mut s: String = chars[..i].iter().collect();
                s.push(t);
                s.extend(&chars[i..]);
                let _ = extract("verification code", &s);
                let _ = extract(&s, &s);
                let _ = html_to_text(&s);
            }
        }
    }

    #[test]
    fn html_to_text_strips_markup() {
        let html = "<html><head><style>p{color:red}</style></head><body><p>Your code&nbsp;is</p><div><b>482913</b></div>&#169; 2026</body></html>";
        let text = html_to_text(html);
        assert!(text.contains("Your code is"));
        assert!(text.contains("482913"));
        assert!(!text.contains("color:red"));
        assert!(text.contains('\u{a9}'));
        assert_eq!(code("Sign in", &text), Some("482913".into()));
    }
}
