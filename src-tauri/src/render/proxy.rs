//! Remote image proxy. When the user chooses "Show images", Rust downloads
//! them and the email frame only ever sees `data:` URLs, so the frame itself
//! never touches the network.
//!
//! - No cookies (no cookie store), no `Referer`, a generic User-Agent.
//! - Public hosts only: IP literals and DNS answers in private, loopback,
//!   link-local, CGNAT or multicast ranges are refused (no LAN probing), and
//!   only ports 80/443.
//! - Images only (sniffed from the bytes, not trusted from the header),
//!   at most 5 MB each, 25 MB and 40 images per message, 10 s timeout,
//!   3 redirects.
//! - Small in-memory cache so re-rendering (theme switch, reopen) is instant.

use base64::Engine as _;
use futures::StreamExt;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;
const MAX_TOTAL_BYTES: usize = 25 * 1024 * 1024;
const MAX_IMAGES: usize = 40;
const CONCURRENCY: usize = 6;
const CACHE_BYTES: usize = 48 * 1024 * 1024;

pub struct ImageProxy {
    http: reqwest::Client,
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    map: HashMap<String, String>,
    bytes: usize,
}

impl ImageProxy {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) PulseMail-ImageProxy")
            .dns_resolver(Arc::new(PublicOnly))
            .redirect(reqwest::redirect::Policy::custom(|attempt| {
                if attempt.previous().len() >= 3 || !url_allowed(attempt.url()) {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .referer(false)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()
            .expect("image proxy client");
        Self {
            http,
            cache: Mutex::new(Cache::default()),
        }
    }

    /// Fetch many images; failures are simply missing from the result.
    pub async fn fetch_all(&self, urls: &[String]) -> HashMap<String, String> {
        let mut out = HashMap::new();
        let mut total = 0usize;
        let mut results = futures::stream::iter(urls.iter().take(MAX_IMAGES).cloned())
            .map(|u| async move {
                let r = self.fetch(&u).await;
                (u, r)
            })
            .buffer_unordered(CONCURRENCY);
        while let Some((u, r)) = results.next().await {
            if let Some(data) = r {
                total += data.len();
                if total > MAX_TOTAL_BYTES * 4 / 3 {
                    break;
                }
                out.insert(u, data);
            }
        }
        out
    }

    async fn fetch(&self, url: &str) -> Option<String> {
        if let Some(hit) = self.cache.lock().ok()?.map.get(url) {
            return Some(hit.clone());
        }
        let parsed = url::Url::parse(url).ok()?;
        if !url_allowed(&parsed) {
            return None;
        }
        let resp = self
            .http
            .get(parsed)
            .header(reqwest::header::ACCEPT, "image/avif,image/webp,image/png,image/*;q=0.8")
            .send()
            .await
            .ok()?;
        if !resp.status().is_success() || resp.content_length().is_some_and(|n| n as usize > MAX_IMAGE_BYTES) {
            return None;
        }
        let mut resp = resp;
        let mut body = Vec::new();
        while let Some(chunk) = resp.chunk().await.ok()? {
            body.extend_from_slice(&chunk);
            if body.len() > MAX_IMAGE_BYTES {
                return None;
            }
        }
        let mime = sniff_image(&body)?;
        let data = format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&body)
        );
        if let Ok(mut c) = self.cache.lock() {
            if c.bytes + data.len() > CACHE_BYTES {
                *c = Cache::default();
            }
            c.bytes += data.len();
            c.map.insert(url.to_string(), data.clone());
        }
        Some(data)
    }
}

/// Scheme, port and (for IP literals) address checks.
pub fn url_allowed(u: &url::Url) -> bool {
    if !matches!(u.scheme(), "http" | "https") || !matches!(u.port_or_known_default(), Some(80 | 443)) {
        return false;
    }
    if !u.username().is_empty() || u.password().is_some() {
        return false;
    }
    match u.host() {
        Some(url::Host::Domain(d)) => {
            let d = d.to_ascii_lowercase();
            !(d == "localhost" || d.ends_with(".localhost") || d.ends_with(".local") || d.ends_with(".internal") || d.ends_with(".lan"))
        }
        Some(url::Host::Ipv4(ip)) => is_public(IpAddr::V4(ip)),
        Some(url::Host::Ipv6(ip)) => is_public(IpAddr::V6(ip)),
        None => false,
    }
}

pub fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_unspecified()
                || v4.is_multicast()
                || o[0] == 0
                || (o[0] == 100 && (64..128).contains(&o[1])) // CGNAT
                || (o[0] == 198 && (o[1] == 18 || o[1] == 19)) // benchmarking
                || o[0] >= 240)
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_public(IpAddr::V4(v4));
            }
            let s = v6.segments();
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                || (s[0] & 0xfe00) == 0xfc00 // unique local
                || (s[0] & 0xffc0) == 0xfe80 // link local
                || (s[0] == 0x2001 && s[1] == 0x0db8)) // documentation
        }
    }
}

/// Resolver that drops non-public answers, so a public name pointing at a
/// LAN address can't be used to reach it.
struct PublicOnly;

impl reqwest::dns::Resolve for PublicOnly {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_string();
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0))
                .await?
                .filter(|a| is_public(a.ip()))
                .collect();
            if addrs.is_empty() {
                return Err("address not allowed".into());
            }
            Ok(Box::new(addrs.into_iter()) as reqwest::dns::Addrs)
        })
    }
}

/// Image type from magic bytes. SVG is refused (it is a document format).
pub fn sniff_image(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else if b.len() > 12 && &b[4..8] == b"ftyp" && (&b[8..12] == b"avif" || &b[8..12] == b"avis") {
        Some("image/avif")
    } else if b.starts_with(b"BM") {
        Some("image/bmp")
    } else if b.starts_with(&[0, 0, 1, 0]) {
        Some("image/x-icon")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_private_and_odd_targets() {
        for u in [
            "http://127.0.0.1/a.png",
            "http://10.0.0.5/a.png",
            "http://192.168.1.1/a.png",
            "http://169.254.169.254/latest/meta-data",
            "http://[::1]/a.png",
            "http://[fd00::1]/a.png",
            "http://[::ffff:192.168.0.1]/a.png",
            "http://100.64.0.1/a.png",
            "http://localhost/a.png",
            "http://printer.local/a.png",
            "https://example.com:8443/a.png",
            "https://user:pw@example.com/a.png",
            "file:///C:/Windows/win.ini",
            "ftp://example.com/a.png",
        ] {
            assert!(!url_allowed(&url::Url::parse(u).unwrap()), "{u} should be blocked");
        }
        for u in ["https://example.com/a.png", "http://8.8.8.8/a.gif", "https://cdn.example.com:443/x"] {
            assert!(url_allowed(&url::Url::parse(u).unwrap()), "{u} should be allowed");
        }
    }

    #[test]
    fn sniffs_real_images_only() {
        assert_eq!(sniff_image(b"\x89PNG\r\n\x1a\n...."), Some("image/png"));
        assert_eq!(sniff_image(b"GIF89a...."), Some("image/gif"));
        assert_eq!(sniff_image(b"<svg onload=alert(1)>"), None);
        assert_eq!(sniff_image(b"<html>"), None);
    }
}
