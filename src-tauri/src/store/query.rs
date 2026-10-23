//! Search query parsing: a Gmail-style query (`from:ana has:attachment
//! "q4 report"`) turned into a safe local FTS5 match plus SQL filters.
//! The same text goes to Gmail as-is for server search; locally we support a
//! subset and report when the query needs the server.

#[derive(Debug, Default, PartialEq, Eq)]
pub struct LocalQuery {
    /// FTS5 MATCH expression ("" = no text condition). Every term is quoted,
    /// so user input can never inject FTS operators.
    pub fts: String,
    pub unread: Option<bool>,
    pub starred: bool,
    pub attachment: bool,
    /// False if the query uses an operator we can't evaluate locally
    /// (`in:`, `label:`, `before:`, `-x`, `OR` …): local results would be
    /// wrong, so only Gmail's answer is used.
    pub local_ok: bool,
}

impl LocalQuery {
    pub fn is_empty(&self) -> bool {
        self.fts.is_empty() && self.unread.is_none() && !self.starred && !self.attachment
    }
}

/// Split on whitespace, keeping `"quoted phrases"` (also `from:"a b"`) together.
fn tokens(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    for ch in input.chars() {
        match ch {
            '"' => {
                quoted = !quoted;
                cur.push(ch);
            }
            c if c.is_whitespace() && !quoted => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// One FTS5 string literal: quotes removed, wrapped in quotes; `*` = prefix.
fn lit(s: &str, prefix: bool) -> Option<String> {
    let w: String = s.chars().filter(|c| *c != '"').collect();
    let w = w.trim();
    if w.is_empty() {
        return None;
    }
    Some(if prefix {
        format!("\"{w}\"*")
    } else {
        format!("\"{w}\"")
    })
}

pub fn parse(input: &str) -> LocalQuery {
    let mut q = LocalQuery {
        local_ok: true,
        ..Default::default()
    };
    let toks = tokens(input);
    let n = toks.len().min(16);
    let mut parts: Vec<String> = Vec::new();
    for (i, t) in toks.into_iter().take(16).enumerate() {
        let last = i + 1 == n;
        let lower = t.to_lowercase();
        if t.starts_with('-') && t.len() > 1 || t == "OR" || t.starts_with('{') || t.starts_with('(') {
            q.local_ok = false;
            continue;
        }
        if let Some((op, val)) = lower
            .split_once(':')
            .filter(|(op, _)| !op.is_empty() && op.chars().all(|c| c.is_ascii_alphabetic() || c == '_'))
        {
            let raw_val = &t[op.len() + 1..];
            match (op, val) {
                ("from", _) => match lit(raw_val, true) {
                    Some(l) => parts.push(format!("{{from_name from_email}} : {l}")),
                    None => q.local_ok = false,
                },
                ("subject", _) => match lit(raw_val, true) {
                    Some(l) => parts.push(format!("subject : {l}")),
                    None => q.local_ok = false,
                },
                ("is", "unread") => q.unread = Some(true),
                ("is", "read") => q.unread = Some(false),
                ("is", "starred") => q.starred = true,
                ("has", "attachment") => q.attachment = true,
                // http://… or "re:" in a subject: plain text, not an operator.
                ("http" | "https" | "re" | "fwd" | "fw", _) => parts.extend(lit(&t, last && !t.ends_with('"'))),
                _ => q.local_ok = false,
            }
            continue;
        }
        let phrase = t.starts_with('"');
        parts.extend(lit(&t, last && !phrase));
    }
    q.fts = parts.join(" ");
    q
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_words_and_phrases() {
        assert_eq!(parse("q4 rep").fts, r#""q4" "rep"*"#);
        assert_eq!(parse(r#""final numbers" sara"#).fts, r#""final numbers" "sara"*"#);
        assert!(parse(r#"a" OR "b"#).local_ok, "stray quotes stay literal");
        assert!(parse("   ").is_empty());
    }

    #[test]
    fn supported_operators() {
        let q = parse("from:github is:unread has:attachment code");
        assert_eq!(q.fts, r#"{from_name from_email} : "github"* "code"*"#);
        assert_eq!(q.unread, Some(true));
        assert!(q.attachment && q.local_ok);
        let q = parse(r#"subject:"q4 report" is:starred"#);
        assert_eq!(q.fts, r#"subject : "q4 report"*"#);
        assert!(q.starred);
        assert_eq!(parse("is:read").unread, Some(false));
    }

    #[test]
    fn server_only_operators() {
        for s in [
            "in:sent hello",
            "label:clients",
            "before:2024/01/01",
            "-promo",
            "a OR b",
            "older_than:1y",
        ] {
            assert!(!parse(s).local_ok, "{s}");
        }
        assert!(parse("https://example.com").local_ok);
    }

    #[test]
    fn injection_is_quoted() {
        // Everything the user types ends up inside FTS string literals.
        let q = parse(r#"NEAR(a b) subject:x"* body:y"#);
        assert_eq!(q.fts, r#""NEAR(a" "b)" subject : "x* body:y"*"#);
        assert!(!parse("body:y").local_ok, "unknown operator");
    }
}
