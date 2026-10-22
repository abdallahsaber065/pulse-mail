//! Optional Google identity data. Tests use synthetic fixtures only.
use super::oauth::AuthError;
use super::secret::Secret;
use base64::Engine as _;
use serde::Deserialize;
use std::time::Duration;
use url::Url;

const MAX_PROFILE_BYTES: usize = 16 * 1024;
const MAX_PHOTO_BYTES: usize = 256 * 1024;
const USERINFO_URL: &str = "https://openidconnect.googleapis.com/v1/userinfo";

pub struct Profile {
    pub name: String,
    pub avatar_data: Option<String>,
}

/// Optional and bounded: only called after an explicit browser sign-in.
/// The OAuth bearer is sent to UserInfo, never to the picture host.
pub async fn fetch(token: &Secret) -> Result<Profile, AuthError> {
    let http = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .referer(false)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(8))
        .build()?;
    let response = http.get(USERINFO_URL).bearer_auth(token.expose()).send().await?;
    let info = parse_userinfo(&read_limited(response, MAX_PROFILE_BYTES).await?)?;
    let avatar_data = match info.picture.as_deref().and_then(picture_url) {
        Some(url) => match http.get(url).send().await {
            Ok(response) => read_limited(response, MAX_PHOTO_BYTES)
                .await
                .ok()
                .and_then(|b| photo_data(&b)),
            Err(_) => None,
        },
        None => None,
    };
    Ok(Profile {
        name: info.name,
        avatar_data,
    })
}

async fn read_limited(mut response: reqwest::Response, max: usize) -> Result<Vec<u8>, AuthError> {
    if !response.status().is_success() || response.content_length().is_some_and(|n| n > max as u64) {
        return Err(AuthError::Protocol("profile download unavailable".into()));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > max {
            return Err(AuthError::Protocol("profile download too large".into()));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[derive(Deserialize)]
struct UserInfo {
    sub: String,
    #[serde(default)]
    name: String,
    picture: Option<String>,
}

fn parse_userinfo(bytes: &[u8]) -> Result<UserInfo, AuthError> {
    let invalid = || AuthError::Protocol("invalid profile response".into());
    if bytes.len() > MAX_PROFILE_BYTES {
        return Err(invalid());
    }
    let info: UserInfo = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    if info.sub.is_empty() || info.sub.len() > 255 || !info.sub.is_ascii() || info.sub.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(info)
}

fn picture_url(value: &str) -> Option<Url> {
    let url = Url::parse(value).ok()?;
    (url.scheme() == "https"
        && url.port_or_known_default() == Some(443)
        && url.username().is_empty()
        && url.password().is_none()
        && url.host_str().is_some_and(|h| h.ends_with(".googleusercontent.com")))
    .then_some(url)
}

fn photo_data(bytes: &[u8]) -> Option<String> {
    if bytes.len() > MAX_PHOTO_BYTES {
        return None;
    }
    let mime = crate::render::proxy::sniff_image(bytes)?;
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_response_reads_optional_name_and_photo() {
        let info = parse_userinfo(
            br#"{"sub":"12345","name":"Alex Saber","picture":"https://lh3.googleusercontent.com/a/avatar"}"#,
        )
        .unwrap();
        assert_eq!(info.name, "Alex Saber");
        assert_eq!(
            info.picture.as_deref(),
            Some("https://lh3.googleusercontent.com/a/avatar")
        );
        let minimal = parse_userinfo(br#"{"sub":"12345"}"#).unwrap();
        assert_eq!(minimal.name, "");
        assert!(minimal.picture.is_none());
    }

    #[test]
    fn malformed_or_oversized_profiles_are_rejected_without_echoing_content() {
        for bytes in [
            b"private invalid response".as_slice(),
            br#"{"sub":""}"#,
            br#"{"name":"Missing id"}"#,
        ] {
            let err = parse_userinfo(bytes).err().expect("invalid profile");
            assert_eq!(
                err.to_string(),
                "unexpected response from Google (invalid profile response)"
            );
        }
        assert!(parse_userinfo(&vec![b' '; MAX_PROFILE_BYTES + 1]).is_err());
    }

    #[test]
    fn photos_are_limited_to_google_https_hosts() {
        for url in [
            "https://lh3.googleusercontent.com/a/avatar",
            "https://lh4.googleusercontent.com:443/a/photo",
        ] {
            assert!(picture_url(url).is_some(), "{url}");
        }
        for url in [
            "http://lh3.googleusercontent.com/a/photo",
            "https://lh3.googleusercontent.com:444/photo",
            "https://googleusercontent.com.evil.example/photo",
            "https://evilgoogleusercontent.com/photo",
            "https://lh3.googleusercontent.com@evil.example/photo",
            "https://user@lh3.googleusercontent.com/photo",
            "https://127.0.0.1/photo",
            "file:///photo",
            "data:image/png;base64,AAAA",
        ] {
            assert!(picture_url(url).is_none(), "{url}");
        }
    }

    #[test]
    fn only_bounded_raster_photos_become_data_urls() {
        let png = base64::engine::general_purpose::STANDARD
            .decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aY1cAAAAASUVORK5CYII=")
            .unwrap();
        assert!(photo_data(&png).unwrap().starts_with("data:image/png;base64,"));
        assert!(photo_data(b"<svg xmlns='http://www.w3.org/2000/svg'></svg>").is_none());
        assert!(photo_data(b"<html>Not a photo</html>").is_none());
        assert!(photo_data(&[png.as_slice(), &vec![0; MAX_PHOTO_BYTES]].concat()).is_none());
    }
}
