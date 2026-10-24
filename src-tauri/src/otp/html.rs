//! HTML signals for the OTP scorer: which tokens the sender made stand out
//! (bold, big, letter-spaced, monospace, a table cell of their own) and which
//! links the mail contains with their anchor text.

use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

use super::normalize;

/// Alphanumeric tokens (separators removed) that the HTML emphasizes.
pub fn emphasized(html: &str) -> HashSet<String> {
    static TAGGED: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?is)<(?:b|strong|h[1-3]|code|pre|tt|kbd|samp|big)\b[^>]*>(.{1,160}?)</(?:b|strong|h[1-3]|code|pre|tt|kbd|samp|big)\s*>",
        )
        .unwrap()
    });
    static STYLED: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"(?is)<(?:td|div|span|p|font|th|center|a)\b([^>]*)>([^<]{3,60})<").unwrap());
    static LOUD_STYLE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(
            r"(?i)font-size\s*:\s*(?:1[89]|[2-9]\d)(?:\.\d+)?px|font-size\s*:\s*(?:1\.[2-9]|[2-9](?:\.\d+)?)(?:em|rem)|letter-spacing|monospace|courier|consolas|menlo|font-weight\s*:\s*(?:bold|[6-9]00)|size\s*=\s*.?[4-7]",
        )
        .unwrap()
    });
    static CELL: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?is)<td\b[^>]*>\s*([A-Za-z0-9\u{0660}-\u{0669}][A-Za-z0-9 \u{0660}-\u{0669}-]{2,14})\s*</td>")
            .unwrap()
    });
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());

    let mut out = HashSet::new();
    let mut add = |s: &str| {
        let t = normalize(&TAG.replace_all(s, " "));
        // Whole text when short ("123 456"), plus each word.
        let joined: String = t.chars().filter(char::is_ascii_alphanumeric).collect();
        if (4..=10).contains(&joined.len()) {
            out.insert(joined);
        }
        for w in t.split(|c: char| !c.is_ascii_alphanumeric()) {
            if (4..=10).contains(&w.len()) {
                out.insert(w.to_string());
            }
        }
    };
    for c in TAGGED.captures_iter(html) {
        add(&c[1]);
    }
    for c in STYLED.captures_iter(html) {
        if LOUD_STYLE.is_match(&c[1]) {
            add(&c[2]);
        }
    }
    for c in CELL.captures_iter(html) {
        add(&c[1]);
    }
    out
}

/// (url, anchor text) for every http(s) link.
pub fn links(html: &str) -> Vec<(String, String)> {
    static A: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"(?is)<a\b[^>]*?href\s*=\s*["'](https?://[^"']+)["'][^>]*>(.{0,300}?)</a\s*>"#).unwrap()
    });
    static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());
    A.captures_iter(html)
        .map(|c| {
            let url = c[1].replace("&amp;", "&");
            let text = TAG
                .replace_all(&c[2], " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            (url, text)
        })
        .collect()
}
