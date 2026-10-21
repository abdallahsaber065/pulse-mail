//! One-time-code (OTP / verification code) extraction.
//!
//! Pure, synchronous and allocation-light so it can run on every incoming
//! message. The extractor is deliberately conservative: a wrong "Copy code"
//! button is worse than no button, so a candidate needs both an OTP-ish
//! message (gating keyword) and a high enough score.

use regex::Regex;
use serde::Serialize;
use std::sync::LazyLock;

/// Only the first part of a body is scanned; codes live near the top.
const MAX_SCAN_CHARS: usize = 4000;
/// Minimum score for a candidate to be reported.
const THRESHOLD: i32 = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OtpMatch {
    /// Normalized code (separators removed), ready for the clipboard.
    pub code: String,
    pub score: i32,
}

/// The message must mention at least one of these to be considered at all.
static GATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:code|codes|otp|passcode|pin|verification|verify|one[- ]?time|2fa|two[- ]factor|mfa)\b|رمز|كود|التحقق",
    )
    .unwrap()
});

/// Words that make a nearby number likely to be the code.
static NEAR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:code|otp|passcode|pin|verification|verify|one[- ]?time|2fa|mfa|security|login|log[- ]?in|sign[- ]?in|authentication|confirmation|confirm)\b|رمز|كود|التحقق",
    )
    .unwrap()
});

/// Context that means the number is NOT a login code.
static NEGATIVE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:promo|coupon|discount|voucher|off|sale|order|invoice|tracking|reference|ref|total|amount|price|phone|call|tel|fax|zip|postal|account (?:number|no))\b",
    )
    .unwrap()
});

/// Candidate tokens: optional short prefix like `G-`, then
/// `123 456` / `123-456`, plain 4-8 digits, or 5-8 uppercase alphanumerics.
static CANDIDATE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:[A-Z]{1,3}-)?(\d{3,4}[ -]\d{3,4}|\d{4,8}|[A-Z0-9]{5,8})\b").unwrap());

static YEAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:19|20)\d{2}$").unwrap());

/// Extract the most likely one-time code from a message.
///
/// `body` should be plain text; use [`html_to_text`] first for HTML-only mail.
pub fn extract(subject: &str, body: &str) -> Option<OtpMatch> {
    let subject = normalize_digits(subject);
    let body: String = normalize_digits(body).chars().take(MAX_SCAN_CHARS).collect();
    let text = format!("{subject}\n{body}");

    if !GATE.is_match(&text) {
        return None;
    }

    let near: Vec<(usize, usize)> = NEAR.find_iter(&text).map(|m| (m.start(), m.end())).collect();
    let subject_end = subject.len();

    let mut best: Option<(i32, usize, String)> = None;
    for caps in CANDIDATE.captures_iter(&text) {
        let whole = caps.get(0).unwrap();
        let core = caps.get(1).unwrap();
        let raw = core.as_str();
        let code: String = raw.chars().filter(|c| c.is_ascii_alphanumeric()).collect();

        let digits_only = code.chars().all(|c| c.is_ascii_digit());
        let has_sep = raw.len() != code.len();
        let mut score = if digits_only {
            match (code.len(), has_sep) {
                (6, true) => 25,
                (6, false) => 30,
                (4 | 5, _) => 20,
                (7 | 8, _) => 20,
                _ => 0,
            }
        } else {
            // Mixed alphanumerics only; plain uppercase words are never codes.
            let has_digit = code.chars().any(|c| c.is_ascii_digit());
            let has_alpha = code.chars().any(|c| c.is_ascii_alphabetic());
            if !(has_digit && has_alpha) {
                continue;
            }
            15
        };
        if score == 0 {
            continue;
        }

        // Keyword proximity (measured in characters, not bytes).
        let start = whole.start();
        let end = whole.end();
        let before_kw = near
            .iter()
            .any(|&(_, kw_end)| kw_end <= start && char_distance(&text, kw_end, start) <= 80);
        let after_kw = near
            .iter()
            .any(|&(kw_start, _)| kw_start >= end && char_distance(&text, end, kw_start) <= 40);
        if before_kw {
            score += 40;
        } else if after_kw {
            score += 20;
        }
        if start < subject_end {
            score += 15;
        }

        // Penalties.
        if digits_only && code.len() == 4 && YEAR.is_match(&code) {
            score -= 30;
        }
        let before = window_before(&text, start, 40);
        let after = window_after(&text, end, 20);
        if NEGATIVE.is_match(before) || NEGATIVE.is_match(after) {
            score -= 60;
        }
        if looks_numeric_context(&text, start, end) {
            score -= 50;
        }
        if in_url_token(&text, start, end) {
            score -= 60;
        }

        if score >= THRESHOLD && best.as_ref().is_none_or(|(s, _, _)| score > *s) {
            best = Some((score, start, code));
        }
    }

    best.map(|(score, _, code)| OtpMatch { code, score })
}

/// Map Arabic-Indic and Eastern Arabic-Indic digits to ASCII.
fn normalize_digits(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{0660}'..='\u{0669}' => char::from(b'0' + (c as u32 - 0x0660) as u8),
            '\u{06F0}'..='\u{06F9}' => char::from(b'0' + (c as u32 - 0x06F0) as u8),
            _ => c,
        })
        .collect()
}

fn char_distance(text: &str, a: usize, b: usize) -> usize {
    text[a..b].chars().count()
}

fn window_before(text: &str, idx: usize, chars: usize) -> &str {
    let start = text[..idx]
        .char_indices()
        .rev()
        .nth(chars.saturating_sub(1))
        .map(|(i, _)| i)
        .unwrap_or(0);
    &text[start..idx]
}

fn window_after(text: &str, idx: usize, chars: usize) -> &str {
    let end = text[idx..]
        .char_indices()
        .nth(chars)
        .map(|(i, _)| idx + i)
        .unwrap_or(text.len());
    &text[idx..end]
}

/// Decimals, money, percentages, times and phone-number fragments.
fn looks_numeric_context(text: &str, start: usize, end: usize) -> bool {
    let prev: Vec<char> = text[..start].chars().rev().take(4).collect();
    let next: Vec<char> = text[end..].chars().take(2).collect();

    let p0 = prev.first().copied();
    let p1 = prev.get(1).copied();
    let n0 = next.first().copied();
    let n1 = next.get(1).copied();

    let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
    // 12.50 / 1,000 / 10:30
    if matches!(n0, Some('.' | ',' | ':')) && digit(n1) {
        return true;
    }
    if matches!(p0, Some('.' | ',' | ':')) && digit(p1) {
        return true;
    }
    // 50% / $49 / €10 / +20 123 ...
    if matches!(n0, Some('%')) {
        return true;
    }
    if prev
        .iter()
        .take(2)
        .any(|c| matches!(c, '$' | '€' | '£' | '¥' | '+' | '#'))
    {
        return true;
    }
    // "1 800 555": a digit group right before (phone-like)
    if p0 == Some(' ') && digit(p1) {
        return true;
    }
    let before = window_before(text, start, 4).to_ascii_uppercase();
    ["EGP", "USD", "EUR", "SAR", "AED"].iter().any(|c| before.contains(c))
}

fn in_url_token(text: &str, start: usize, end: usize) -> bool {
    let tok_start = text[..start].rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
    let tok_end = text[end..]
        .find(char::is_whitespace)
        .map(|i| end + i)
        .unwrap_or(text.len());
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
