//! Email rendering: turns a cached body into a self-contained, sanitized
//! `srcdoc` for the reader's sandboxed iframe.
//!
//! Layers (any one of them alone should stop an attack):
//! 1. **Sanitizer** (ammonia allowlist): no scripts, event handlers, forms,
//!    frames, objects, `<meta>`/`<base>`/`<link>`; links limited to
//!    http/https/mailto; images limited to `data:` (remote and `cid:` images
//!    are rewritten to local `data:` URLs or a blank placeholder); CSS cleaned
//!    by [`css`].
//! 2. **CSP** inside the document: `default-src 'none'`, images and fonts only
//!    from `data:`, and the only script allowed is our own frame script,
//!    pinned by its SHA-256 hash (it reports height and link clicks).
//! 3. **Sandbox**: the iframe gets `sandbox="allow-scripts"` *without*
//!    `allow-same-origin`, so the document runs in an opaque origin: it can't
//!    touch the app, its storage or the IPC bridge, can't navigate the app,
//!    open popups or submit forms.
//!
//! The frame never touches the network: remote images, when the user allows
//! them, are fetched by Rust ([`proxy`]) and inlined as `data:` URLs.

pub mod css;
pub mod proxy;

use base64::Engine as _;
use regex::Regex;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock};

/// 1x1 transparent GIF: stands in for any image we won't load.
pub const BLANK_GIF: &str = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";

/// The only script that may run in the email frame. It
/// - reports the content height so the iframe can grow (no inner scrollbar),
/// - reports link clicks so the app can open them in the system browser
///   (after a phishing check), and
/// - collapses quoted text behind a "•••" toggle, like Gmail.
///
/// Its SHA-256 is pinned in the frame CSP and in the app CSP (tauri.conf.json);
/// a unit test fails if they drift apart.
pub const FRAME_SCRIPT: &str = r#"(function(){var d=document,P=parent;function post(m){m.pm=1;P.postMessage(m,"*");}function h(){var e=d.documentElement,b=d.body;post({t:"h",v:Math.ceil(Math.max(e.scrollHeight,b?b.scrollHeight:0))});}var qs=d.querySelectorAll(".gmail_quote,blockquote[type=cite],.yahoo_quoted,.moz-cite-prefix+blockquote,.pm-quote"),top=[];for(var i=0;i<qs.length;i++){var q=qs[i],p=q.parentElement,inner=false;while(p){if(top.indexOf(p)>=0){inner=true;break;}p=p.parentElement;}if(inner||/Forwarded message|Original Message/i.test((q.textContent||"").slice(0,400)))continue;top.push(q);q.classList.add("pm-q");var b=d.createElement("button");b.className="pm-qt";b.type="button";b.title="Show trimmed content";b.setAttribute("aria-expanded","false");b.textContent="\u2022\u2022\u2022";q.parentNode.insertBefore(b,q);}d.addEventListener("click",function(e){var t=e.target,b=t&&t.closest?t.closest(".pm-qt"):null;if(b){e.preventDefault();var n=b.nextElementSibling,o=n&&n.classList.toggle("pm-open");b.setAttribute("aria-expanded",o?"true":"false");h();return;}var a=t&&t.closest?t.closest("a[href]"):null;if(a){e.preventDefault();post({t:"link",href:a.getAttribute("href")||"",text:(a.textContent||"").trim().slice(0,300)});}},true);if(window.ResizeObserver)new ResizeObserver(h).observe(d.documentElement);window.addEventListener("load",h);h();})();"#;

/// `'sha256-…'` source expression for [`FRAME_SCRIPT`].
pub fn frame_script_csp_hash() -> String {
    let digest = Sha256::digest(FRAME_SCRIPT.as_bytes());
    format!("'sha256-{}'", base64::engine::general_purpose::STANDARD.encode(digest))
}

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderOptions {
    pub dark: bool,
    /// In dark mode, show the email's own (light) colors instead of the
    /// smart-inverted version.
    pub original_colors: bool,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Rendered {
    pub srcdoc: String,
    /// Remote (non-tracker) images in the email.
    pub remote_images: usize,
    /// Of those, how many are shown (fetched through the proxy).
    pub images_shown: usize,
    /// Tracking pixels found; never loaded, even when images are shown.
    pub trackers: usize,
    pub plain_text: bool,
}

/// What a pre-scan of the raw HTML found.
#[derive(Debug, Default)]
pub struct Scan {
    /// Contents of all `<style>` elements, already sanitized.
    pub stylesheet: String,
    /// Remote image URLs that look like real content, in document order.
    pub remote: Vec<String>,
    /// Remote image URLs that look like tracking pixels.
    pub trackers: HashSet<String>,
}

static STYLE_BLOCK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<style\b[^>]*>(.*?)</style\s*>").unwrap());
static IMG_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<img\b[^>]*>").unwrap());
static ATTR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)[\s/](src|width|height|style)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#).unwrap()
});

/// URL fragments used by common mail open-tracking services.
const TRACKER_HINTS: &[&str] = &[
    "/track/open",
    "/wf/open",
    "/trk/open",
    "/open.aspx",
    "/open.gif",
    "/open?",
    "/o.gif",
    "/pixel.gif",
    "/pixel.png",
    "/pixel?",
    "/beacon",
    "/e/o/",
    "/tr/op/",
    "/emimp/",
    "mailtrack.io",
    "mandrillapp.com/track",
    "list-manage.com/track",
    "sparkpostmail.com/q/",
    "hubspotemail.net/e2t/to",
    "google-analytics.com/collect",
    "mktdns.com/",
    "email.mg.",
    "/imp?",
    "/open/",
];

pub fn scan(html: &str) -> Scan {
    let mut out = Scan::default();
    let mut css = String::new();
    for c in STYLE_BLOCK.captures_iter(html) {
        css.push_str(&c[1]);
        css.push('\n');
    }
    out.stylesheet = css::sanitize_stylesheet(&css);
    let mut seen = HashSet::new();
    for tag in IMG_TAG.find_iter(html) {
        let (mut src, mut w, mut h, mut style) = (None, None, None, String::new());
        for a in ATTR.captures_iter(tag.as_str()) {
            let v = a.get(2).or(a.get(3)).or(a.get(4)).map(|m| decode_entities(m.as_str()));
            match a[1].to_ascii_lowercase().as_str() {
                "src" => src = v,
                "width" => w = v,
                "height" => h = v,
                _ => style = v.unwrap_or_default(),
            }
        }
        let Some(src) = src.map(|s| s.trim().to_string()) else {
            continue;
        };
        if !is_remote(&src) {
            continue;
        }
        if is_tracker(&src, w.as_deref(), h.as_deref(), &style) {
            out.trackers.insert(src);
        } else if seen.insert(src.clone()) {
            out.remote.push(src);
        }
    }
    // A URL used both as a pixel and as a real image is treated as a pixel.
    out.remote.retain(|u| !out.trackers.contains(u));
    out
}

fn is_remote(url: &str) -> bool {
    let l = url.get(..8).unwrap_or(url).to_ascii_lowercase();
    l.starts_with("https://") || l.starts_with("http://")
}

fn is_tracker(url: &str, w: Option<&str>, h: Option<&str>, style: &str) -> bool {
    let tiny = |v: Option<&str>| {
        v.map(|s| s.trim().trim_end_matches("px").trim())
            .and_then(|s| s.parse::<f32>().ok())
            .is_some_and(|n| n <= 3.0)
    };
    let st: String = style
        .to_ascii_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let hidden = st.contains("display:none")
        || st.contains("visibility:hidden")
        || st.contains("opacity:0;")
        || st.ends_with("opacity:0")
        || ((st.contains("width:1px") || st.contains("width:0"))
            && (st.contains("height:1px") || st.contains("height:0")));
    let u = url.to_ascii_lowercase();
    (tiny(w) && tiny(h)) || hidden || TRACKER_HINTS.iter().any(|p| u.contains(p))
}

/// Minimal entity decoding for attribute values found by the pre-scan, so
/// they match what the HTML parser (and therefore the sanitizer) sees.
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    s.replace("&amp;", "&")
        .replace("&#38;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// Per-render inputs for the attribute filter.
#[derive(Default)]
pub struct Resources {
    /// Remote URL -> `data:` URL for images fetched through the proxy.
    pub images: HashMap<String, String>,
    /// Normalized Content-ID (no `<>`, lowercase) -> `data:` URL.
    pub cids: HashMap<String, String>,
}

struct FilterCtx {
    res: Resources,
    trackers: HashSet<String>,
    shown: AtomicUsize,
}

/// Sanitize an HTML body. Returns the clean body markup and how many remote
/// images were actually substituted.
pub fn sanitize(html: &str, scan: &Scan, res: Resources) -> (String, usize) {
    let ctx = Arc::new(FilterCtx {
        res,
        trackers: scan.trackers.clone(),
        shown: AtomicUsize::new(0),
    });
    let c = ctx.clone();
    let mut b = ammonia::Builder::default();
    b.add_tags([
        "font", "center", "tfoot", "address", "section", "article", "header", "footer", "main", "aside", "big",
        "small", "u", "s", "strike", "hr", "caption", "colgroup", "col",
    ])
    .add_clean_content_tags([
        "title", "xml", "iframe", "object", "embed", "noembed", "noframes", "applet", "frameset", "textarea", "select",
        "template", "svg", "math", "head",
    ])
    .add_generic_attributes([
        "style",
        "class",
        "id",
        "dir",
        "align",
        "valign",
        "width",
        "height",
        "bgcolor",
        "color",
        "border",
        "cellpadding",
        "cellspacing",
        "face",
        "size",
        "role",
        "aria-label",
        "aria-hidden",
        "colspan",
        "rowspan",
        "nowrap",
        "type",
        "start",
        "alt",
    ])
    .url_schemes(HashSet::from(["http", "https", "mailto", "cid", "data"]))
    .url_relative(ammonia::UrlRelative::Deny)
    .link_rel(Some("noopener noreferrer"))
    .strip_comments(true)
    .attribute_filter(move |el, attr, value| filter_attr(&c, el, attr, value));
    let body = b.clean(html).to_string();
    (body, ctx.shown.load(Ordering::Relaxed))
}

fn filter_attr<'u>(ctx: &FilterCtx, el: &str, attr: &str, value: &'u str) -> Option<Cow<'u, str>> {
    match (el, attr) {
        ("a", "href") => {
            let l = value.trim_start().to_ascii_lowercase();
            (l.starts_with("http://") || l.starts_with("https://") || l.starts_with("mailto:"))
                .then_some(Cow::Borrowed(value))
        }
        ("img", "src") => Some(Cow::Owned(image_src(ctx, value.trim()))),
        (_, "style") => Some(Cow::Owned(css::sanitize_declarations(value))),
        // Any other URL-bearing attribute that slipped through the allowlist.
        (_, "src" | "href" | "background" | "action" | "formaction" | "poster" | "srcset" | "xlink:href") => None,
        _ => Some(Cow::Borrowed(value)),
    }
}

fn image_src(ctx: &FilterCtx, v: &str) -> String {
    if css::is_inline_image(v) {
        return v.to_string();
    }
    if v.len() > 4 && v[..4].eq_ignore_ascii_case("cid:") {
        return ctx
            .res
            .cids
            .get(&normalize_cid(&v[4..]))
            .cloned()
            .unwrap_or_else(|| BLANK_GIF.to_string());
    }
    if is_remote(v)
        && !ctx.trackers.contains(v)
        && let Some(data) = ctx.res.images.get(v)
    {
        ctx.shown.fetch_add(1, Ordering::Relaxed);
        return data.clone();
    }
    BLANK_GIF.to_string()
}

/// `<Part.1@x>` / `part.1@x` -> `part.1@x`; `cid:` URLs are also %-encoded.
pub fn normalize_cid(s: &str) -> String {
    let s = s.trim().trim_start_matches('<').trim_end_matches('>');
    let decoded = url::form_urlencoded::parse(format!("x={s}").as_bytes())
        .next()
        .map(|(_, v)| v.into_owned())
        .unwrap_or_else(|| s.to_string());
    decoded.to_ascii_lowercase()
}

/// Render an HTML body into a complete frame document.
pub fn render_html(html: &str, res: Resources, opts: RenderOptions) -> Rendered {
    let scan = scan(html);
    let (body, shown) = sanitize(html, &scan, res);
    Rendered {
        srcdoc: document(&body, &scan.stylesheet, opts, false),
        remote_images: scan.remote.len(),
        images_shown: shown,
        trackers: scan.trackers.len(),
        plain_text: false,
    }
}

/// Render a plain-text body: escaped, URLs linkified, `>` quotes folded.
pub fn render_text(text: &str, opts: RenderOptions) -> Rendered {
    let body = format!("<div class=\"pm-text\" dir=\"auto\">{}</div>", text_to_html(text));
    Rendered {
        srcdoc: document(&body, "", opts, true),
        plain_text: true,
        ..Default::default()
    }
}

static URL_IN_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\b(?:https?://|www\.)[^\s<>"'\u{00a0}]+"#).unwrap());

/// Plain text -> HTML. Lines starting with `>` become nested
/// `<blockquote class="pm-quote">` so the frame script can collapse them.
pub fn text_to_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 64);
    let mut quote: Vec<&str> = Vec::new();
    let flush = |quote: &mut Vec<&str>, out: &mut String| {
        if quote.is_empty() {
            return;
        }
        let inner: Vec<&str> = quote
            .iter()
            .map(|l| {
                let l = &l[1..];
                l.strip_prefix(' ').unwrap_or(l)
            })
            .collect();
        out.push_str("<blockquote class=\"pm-quote\">");
        out.push_str(&text_to_html(&inner.join("\n")));
        out.push_str("</blockquote>");
        quote.clear();
    };
    let lines: Vec<&str> = text.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.starts_with('>') {
            quote.push(line);
            continue;
        }
        flush(&mut quote, &mut out);
        out.push_str(&linkify(line));
        if i + 1 < lines.len() {
            out.push('\n');
        }
    }
    flush(&mut quote, &mut out);
    out
}

fn linkify(line: &str) -> String {
    let mut out = String::new();
    let mut last = 0;
    for m in URL_IN_TEXT.find_iter(line) {
        let mut url = m.as_str();
        // Trailing punctuation is almost never part of the link.
        while let Some(c) = url.chars().last() {
            if ".,;:!?)]}'\"".contains(c) {
                url = &url[..url.len() - c.len_utf8()];
            } else {
                break;
            }
        }
        if url.is_empty() {
            continue;
        }
        let start = m.start();
        let end = start + url.len();
        out.push_str(&escape(&line[last..start]));
        let href = if url.to_ascii_lowercase().starts_with("www.") {
            format!("https://{url}")
        } else {
            url.to_string()
        };
        out.push_str(&format!(
            "<a href=\"{}\" rel=\"noopener noreferrer\">{}</a>",
            escape(&href),
            escape(url)
        ));
        last = end;
    }
    out.push_str(&escape(&line[last..]));
    out
}

pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            c => o.push(c),
        }
    }
    o
}

/// Frame CSP: nothing but inline styles, `data:` images/fonts and our script.
pub fn frame_csp() -> String {
    format!(
        "default-src 'none'; img-src data:; style-src 'unsafe-inline'; font-src data:; script-src {}; \
         connect-src 'none'; frame-src 'none'; media-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'",
        frame_script_csp_hash()
    )
}

/// Wrap sanitized markup into the frame document.
pub fn document(body: &str, stylesheet: &str, opts: RenderOptions, plain: bool) -> String {
    let smart_dark = opts.dark && !opts.original_colors && !plain;
    let (fg, link, quote_border) = if opts.dark && plain {
        ("#e3e3e3", "#a8c7fa", "#5f6368")
    } else {
        ("#1f1f1f", "#0b57d0", "#c4c7c5")
    };
    let bg = if opts.dark && plain {
        "transparent"
    } else if smart_dark {
        // Inverted below, this becomes the app's dark surface.
        "#e4e2dd"
    } else if opts.dark {
        "#ffffff"
    } else {
        "transparent"
    };
    let dark_css = if smart_dark {
        "html{filter:invert(1) hue-rotate(180deg)}\
         img,video,picture,svg,[style*=\"background-image\"],[style*=\"background:url\"]{filter:invert(1) hue-rotate(180deg)}"
    } else {
        ""
    };
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
<meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\">\
<meta name=\"referrer\" content=\"no-referrer\">\
<meta name=\"color-scheme\" content=\"{scheme}\">\
<style>{stylesheet}</style>\
<style>\
html{{background:{bg};color-scheme:{scheme}}}\
body{{margin:0;padding:{pad};font:14px/1.5 \"Segoe UI\",system-ui,sans-serif;color:{fg};overflow-wrap:anywhere;overflow-y:hidden}}\
a{{color:{link}}}\
img{{max-width:100%;height:auto}}\
img[src^=\"{blank}\"]{{height:revert-layer;background:rgba(127,127,127,.08)}}\
img[src^=\"{blank}\"]:not([height]){{max-height:120px}}\
table{{max-width:100%}}\
pre{{white-space:pre-wrap}}\
p,li,blockquote,pre,h1,h2,h3,h4,h5,h6{{unicode-bidi:plaintext}}\
.pm-text{{white-space:pre-wrap;unicode-bidi:plaintext;text-align:start}}\
blockquote.pm-quote{{margin:4px 0 4px 4px;border-inline-start:2px solid {quote_border};padding-inline-start:10px;opacity:.85}}\
.pm-q{{display:none!important}}.pm-q.pm-open{{display:block!important}}\
.pm-qt{{display:block;border:0;border-radius:6px;background:#e8eaed;color:#3c4043;padding:0 8px;height:14px;line-height:6px;\
font:700 12px/14px sans-serif;letter-spacing:1px;cursor:pointer;margin:8px 0}}\
.pm-qt:hover{{background:#dadce0}}\
{dark_css}\
@media print{{html{{filter:none!important;background:#fff!important}}img{{filter:none!important}}.pm-qt{{display:none}}.pm-q{{display:block!important}}}}\
</style></head><body dir=\"auto\">{body}<script>{script}</script></body></html>",
        csp = frame_csp(),
        scheme = if opts.dark && plain { "dark" } else { "light" },
        pad = if opts.dark && !plain { "12px 14px" } else { "4px 2px" },
        script = FRAME_SCRIPT,
        blank = &BLANK_GIF[..40],
    )
}

#[cfg(test)]
mod tests;
