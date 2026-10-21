//! CSS sanitizing for email `<style>` blocks and `style=""` attributes.
//!
//! This is defense in depth: the email frame's CSP already refuses every
//! remote load (`img-src data:`, `default-src 'none'`). Here we make sure the
//! CSS can't even ask for one, can't run legacy script hooks
//! (`expression()`, `behavior`, `-moz-binding`), can't break out of its
//! `<style>` element, and can't pin content over the reader with
//! `position: fixed`.

use regex::Regex;
use std::sync::LazyLock;

static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)/\*.*?\*/").unwrap());
/// A run of CSS (selector or declaration) that contains a backslash escape.
/// Escapes can spell `url(` or `expression(` in ways simple matching misses,
/// so any declaration with one is dropped whole.
static ESCAPED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^;{}]*\\[^;{}]*").unwrap());
static AT_RULES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)@(?:import|charset|namespace)[^;{]*;?|@font-face\s*\{[^}]*\}").unwrap()
});
static URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)url\s*\(\s*(?:"([^"]*)"|'([^']*)'|([^)'"]*))\s*\)"#).unwrap());
/// A whole declaration that uses a script hook or a URL-taking function we
/// don't rewrite; it is removed entirely.
static DANGEROUS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[^;{}]*(?:expression\s*\(|image-set\s*\(|\bsrc\s*\(|javascript\s*:|vbscript\s*:|behavior\s*:|-moz-binding)[^;{}]*")
        .unwrap()
});
static FIXED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)position\s*:\s*(?:fixed|sticky)").unwrap());

/// Clean a whole stylesheet (the contents of `<style>` elements).
pub fn sanitize_stylesheet(css: &str) -> String {
    let s = COMMENT.replace_all(css, "");
    let s = ESCAPED.replace_all(&s, "");
    let s = AT_RULES.replace_all(&s, "");
    let s = common(&s);
    // Never let the text close the <style> element or open a tag.
    s.replace(['<', '>'], " ")
}

/// Clean an inline `style` attribute value.
pub fn sanitize_declarations(style: &str) -> String {
    let s = COMMENT.replace_all(style, "");
    let kept: Vec<&str> = s.split(';').filter(|d| !d.contains('\\')).collect();
    common(&kept.join(";"))
}

fn common(s: &str) -> String {
    let s = DANGEROUS.replace_all(s, "");
    let s = URL.replace_all(&s, |c: &regex::Captures<'_>| {
        let v = c.get(1).or(c.get(2)).or(c.get(3)).map(|m| m.as_str().trim()).unwrap_or("");
        if is_inline_image(v) {
            format!("url(\"{}\")", v.replace('"', ""))
        } else {
            "none".to_string()
        }
    });
    FIXED.replace_all(&s, "position:static").into_owned()
}

/// `data:image/...` URLs are local bytes; everything else would be a fetch.
pub fn is_inline_image(url: &str) -> bool {
    url.get(..11).is_some_and(|p| p.eq_ignore_ascii_case("data:image/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_urls_become_none() {
        let out = sanitize_declarations("background:url(https://t.example/p.gif) no-repeat; color:red");
        assert!(!out.contains("example"), "{out}");
        assert!(out.contains("color:red"));
        let out = sanitize_stylesheet(r#"td{background-image:url( "http://x.test/a.png" )}"#);
        assert!(!out.contains("x.test"), "{out}");
    }

    #[test]
    fn data_images_are_kept() {
        let out = sanitize_declarations("background:url('data:image/png;base64,AAAA')");
        assert!(out.contains("data:image/png;base64,AAAA"), "{out}");
    }

    #[test]
    fn script_hooks_are_neutralised() {
        for bad in [
            "width:expression(alert(1))",
            "behavior:url(x.htc)",
            "-moz-binding:url(x.xml#xss)",
            "background:image-set('https://x.test/a.png' 1x)",
            "background:-webkit-image-set(url(https://x.test/a.png) 1x)",
            "background:src('https://x.test/a.png')",
            "background:url(javascript:alert(1))",
        ] {
            let out = sanitize_declarations(bad).to_lowercase();
            assert!(!out.contains("expression(") && !out.contains("x.test") && !out.contains("javascript:"), "{bad} -> {out}");
            assert!(!out.contains("-moz-binding") && !out.contains("behavior:"), "{bad} -> {out}");
        }
    }

    #[test]
    fn escapes_drop_the_declaration() {
        let out = sanitize_declarations(r"background:u\72l(https://x.test/a.png);color:blue");
        assert_eq!(out, "color:blue");
        let out = sanitize_stylesheet(r"p{background:\75 rl(https://x.test/a.png)} a{color:red}");
        assert!(!out.contains("x.test"), "{out}");
        assert!(out.contains("color:red"));
    }

    #[test]
    fn imports_fonts_and_fixed_positioning() {
        let out = sanitize_stylesheet(
            "@import url(https://x.test/a.css); @font-face{font-family:x;src:url(https://x.test/f.woff)} div{position:fixed;top:0}",
        );
        assert!(!out.contains("x.test") && !out.contains("@import"), "{out}");
        assert!(out.contains("position:static"), "{out}");
    }

    #[test]
    fn cannot_close_the_style_element() {
        let out = sanitize_stylesheet("a{color:red}</style><script>alert(1)</script>");
        assert!(!out.contains('<'), "{out}");
    }
}
