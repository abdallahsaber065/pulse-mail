use super::*;

fn msg() -> MessageRow {
    MessageRow {
        id: "m1".into(),
        thread_id: "t1".into(),
        internal_date: 1_700_000_000_000,
        from_name: "Ana Lima".into(),
        from_email: "ana@x.test".into(),
        to_list: "me@gmail.com, bob@y.test".into(),
        subject: "Q4 report".into(),
        snippet: String::new(),
        body_text: "Hi,\n\nnumbers attached.".into(),
        body_html: None,
        label_ids: vec![],
        unread: false,
        starred: false,
        has_attachments: false,
        otp_code: None,
        body_loaded: true,
        cc_list: "carol@z.test, ME@gmail.com".into(),
        reply_to: String::new(),
        rfc_message_id: "<abc@x.test>".into(),
        rfc_references: "<root@x.test>".into(),
        attachments: vec![],
    }
}

#[test]
fn recipients_parse_and_reject() {
    let r = parse_recipients(r#""Lima, Ana" <ana@x.test>, bob@y.test; Carol <carol@z.test>"#).unwrap();
    assert_eq!(r.len(), 3);
    assert_eq!(r[0], Recipient { name: "Lima, Ana".into(), email: "ana@x.test".into() });
    assert_eq!(r[1].email, "bob@y.test");
    assert_eq!(r[2].name, "Carol");
    assert!(parse_recipients("  ").unwrap().is_empty());
    for bad in ["not-an-email", "a@b", "x@y.test\r\nBcc: evil@z.test", "<a@b.c", "a@@b.test", "a b@c.test"] {
        assert!(parse_recipients(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn reply_and_reply_all() {
    let m = msg();
    let r = reply_context(&m, "me@gmail.com", Mode::Reply);
    assert_eq!(r.to, "\"Ana Lima\" <ana@x.test>");
    assert_eq!(r.cc, "");
    assert_eq!(r.subject, "Re: Q4 report");
    assert_eq!(r.in_reply_to, "<abc@x.test>");
    assert_eq!(r.references, "<root@x.test> <abc@x.test>");
    assert_eq!(r.thread_id.as_deref(), Some("t1"));

    let all = reply_context(&m, "me@gmail.com", Mode::ReplyAll);
    assert_eq!(all.to, "\"Ana Lima\" <ana@x.test>, bob@y.test");
    assert_eq!(all.cc, "carol@z.test", "my own address is dropped (any case)");

    let mut rt = msg();
    rt.reply_to = "list@x.test".into();
    assert_eq!(reply_context(&rt, "me@gmail.com", Mode::Reply).to, "list@x.test");

    let mut mine = msg();
    mine.from_email = "me@gmail.com".into();
    mine.to_list = "ana@x.test".into();
    assert_eq!(reply_context(&mine, "me@gmail.com", Mode::Reply).to, "ana@x.test", "replying to my own mail");

    let mut re = msg();
    re.subject = "RE: Q4 report".into();
    assert_eq!(reply_context(&re, "me@gmail.com", Mode::Reply).subject, "RE: Q4 report");
}

#[test]
fn forward_has_no_recipients_or_thread() {
    let f = reply_context(&msg(), "me@gmail.com", Mode::Forward);
    assert_eq!(f.subject, "Fwd: Q4 report");
    assert!(f.to.is_empty() && f.thread_id.is_none() && f.in_reply_to.is_empty());
    assert_eq!(f.original_text, "Hi,\n\nnumbers attached.");
}

fn draft() -> Draft {
    Draft {
        to: "Ana <ana@x.test>".into(),
        subject: "Re: Q4\r\nBcc: evil@z.test".into(),
        body: "Thanks <b>Ana</b> & team\nسلام".into(),
        quote: Some(Quote { header: "On Tue, Ana wrote:".into(), text: "line 1\n\n<script>x</script>".into(), forward: false }),
        in_reply_to: "<abc@x.test>".into(),
        references: "<root@x.test> junk <abc@x.test>".into(),
        thread_id: Some("t1".into()),
        ..Default::default()
    }
}

#[test]
fn bodies_escape_and_quote() {
    let d = draft();
    let html = html_body(&d);
    assert!(html.contains("Thanks &lt;b&gt;Ana&lt;/b&gt; &amp; team<br>"));
    assert!(html.contains("&lt;script&gt;x&lt;/script&gt;"));
    assert!(!html.contains("<script>"));
    assert!(html.contains("dir=\"auto\"") && html.contains("blockquote class=\"gmail_quote\""));
    let text = text_body(&d);
    assert!(text.ends_with("On Tue, Ana wrote:\n> line 1\n>\n> <script>x</script>\n"), "{text}");
}

#[test]
fn raw_message_headers_are_safe() {
    let d = draft();
    let p = d.prepare(true).unwrap();
    let raw = String::from_utf8(build_raw(&d, &p, "me@gmail.com", "", vec![])).unwrap();
    let headers = raw.split("\r\n\r\n").next().unwrap();
    assert!(headers.contains("From: <me@gmail.com>") || headers.contains("From: me@gmail.com"), "{headers}");
    assert!(headers.contains("ana@x.test"));
    assert!(headers.contains("In-Reply-To: <abc@x.test>"));
    assert!(headers.contains("<root@x.test>") && !headers.contains("junk"));
    assert!(!headers.lines().any(|l| l.starts_with("Bcc:")), "no header injection through the subject");
    assert!(raw.contains("multipart/alternative"));
}

#[test]
fn prepare_validates() {
    let mut d = draft();
    d.to = String::new();
    assert!(d.prepare(false).is_ok(), "drafts may have no recipients");
    assert_eq!(d.prepare(true).err().unwrap(), "Add at least one recipient.");
    d.to = "ok@x.test, nope".into();
    assert!(d.prepare(false).unwrap_err().starts_with("To: \"nope\""));
    d.to = "ok@x.test".into();
    d.body = "x".repeat(MAX_BODY + 1);
    assert!(d.prepare(true).is_err());
    assert_eq!(message_ids("<a@b> <bad> c@d <e@f>"), ["a@b", "e@f"]);
}

#[test]
fn rich_html_is_sanitized_and_files_attached() {
    let d = Draft {
        to: "ana@x.test".into(),
        subject: "Pulse Mail test".into(),
        body: "Hello bold".into(),
        html: Some(r#"<p dir="auto" style="color:red" onclick="x()">Hello <strong>bold</strong><script>alert(1)</script> <a href="javascript:alert(1)">x</a> <a href="https://ok.test">ok</a><img src="https://t.test/p.gif"></p>"#.into()),
        attachments: vec![DraftAttachment { name: "a.txt".into(), mime: "text/plain".into(), size: 5, staged: Some("1".into()), ..Default::default() }],
        ..Default::default()
    };
    let html = html_body(&d);
    assert!(html.contains("<strong>bold</strong>") && html.contains(r#"<p dir="auto">"#));
    for bad in ["script", "onclick", "style=", "javascript:", "<img"] {
        assert!(!html.contains(bad), "{bad} survived: {html}");
    }
    assert!(html.contains(r#"href="https://ok.test""#));
    let p = d.prepare(true).unwrap();
    let raw = String::from_utf8_lossy(&build_raw(&d, &p, "me@gmail.com", "Me", vec![FilePart { name: "a\r\n.txt".into(), mime: "text/plain\r\nX: y".into(), bytes: b"hello".to_vec() }])).to_string();
    assert!(raw.contains("multipart/mixed") && raw.contains("multipart/alternative"));
    assert!(raw.contains("application/octet-stream"), "unsafe mime replaced");
    assert!(!raw.contains("\r\nX: y"));
    assert!(raw.contains("From: \"Me\" <me@gmail.com>") || raw.contains("From: Me <me@gmail.com>"));
}

#[test]
fn attachment_limits() {
    let mut d = Draft { to: "ana@x.test".into(), ..Default::default() };
    d.attachments = vec![DraftAttachment { name: "big.bin".into(), mime: "application/zip".into(), size: MAX_ATTACH_TOTAL + 1, staged: Some("1".into()), ..Default::default() }];
    assert!(d.prepare(true).unwrap_err().contains("25 MB"));
    d.attachments[0].size = 10;
    d.attachments[0].staged = None;
    assert!(d.prepare(true).is_err(), "attachment without bytes source");
    assert_eq!(safe_mime("Image/PNG"), "image/png");
    assert_eq!(safe_mime("bad"), "application/octet-stream");
}
