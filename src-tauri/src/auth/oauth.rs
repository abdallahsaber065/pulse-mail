//! Google OAuth 2.0 for installed apps: authorization code + PKCE with a
//! loopback redirect on 127.0.0.1 (random port).
//! https://developers.google.com/identity/protocols/oauth2/native-app

use super::client_config::OAuthClient;
use super::pkce;
use super::secret::Secret;
use serde::Deserialize;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use url::Url;

pub const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
pub const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
pub const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
/// Full Gmail access: needed for IMAP (XOAUTH2) and for later actions
/// (archive, mark read, send). Restricted scope -> personal "In production"
/// unverified app; see docs/SETUP-GOOGLE-OAUTH.md.
pub const SCOPE: &str = "https://mail.google.com/ openid profile";
/// How long we wait for the user to finish in the browser.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("sign-in was cancelled or timed out")]
    Cancelled,
    #[error("Google denied access: {0}")]
    Denied(String),
    #[error("sign-in response did not match this request")]
    StateMismatch,
    #[error("Google did not return a refresh token")]
    NoRefreshToken,
    #[error("the saved sign-in is no longer valid")]
    InvalidGrant,
    #[error("network error talking to Google")]
    Network(#[from] reqwest::Error),
    #[error("local sign-in listener failed")]
    Io(#[from] std::io::Error),
    #[error("unexpected response from Google ({0})")]
    Protocol(String),
}

#[derive(Debug, Clone)]
pub struct Tokens {
    pub access_token: Secret,
    pub expires_at: Instant,
    pub refresh_token: Option<Secret>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: Secret,
    expires_in: u64,
    refresh_token: Option<Secret>,
}

#[derive(Deserialize)]
struct TokenErrorResponse {
    error: String,
}

/// A pending browser sign-in: open `auth_url`, then await [`PendingLogin::finish`].
pub struct PendingLogin {
    pub auth_url: String,
    listener: TcpListener,
    redirect_uri: String,
    state: String,
    verifier: String,
}

pub async fn begin_login(client: &OAuthClient, login_hint: Option<&str>) -> Result<PendingLogin, AuthError> {
    // Loopback on IPv4 literal (not "localhost") per Google's guidance.
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let redirect_uri = format!("http://127.0.0.1:{port}");
    let pkce = pkce::generate();
    let state = pkce::random_urlsafe(24);

    let mut url = Url::parse(AUTH_URL).expect("static url");
    url.query_pairs_mut()
        .append_pair("client_id", &client.client_id)
        .append_pair("redirect_uri", &redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", SCOPE)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("state", &state)
        .append_pair("access_type", "offline")
        // Always show consent so Google issues a refresh token every time.
        .append_pair("prompt", "consent");

    if let Some(email) = login_hint {
        url.query_pairs_mut().append_pair("login_hint", email);
    }

    Ok(PendingLogin {
        auth_url: url.into(),
        listener,
        redirect_uri,
        state,
        verifier: pkce.verifier,
    })
}

impl PendingLogin {
    /// Wait for the browser redirect, then exchange the code for tokens.
    pub async fn finish(self, http: &reqwest::Client, client: &OAuthClient) -> Result<Tokens, AuthError> {
        let code = tokio::time::timeout(LOGIN_TIMEOUT, wait_for_code(&self.listener, &self.state))
            .await
            .map_err(|_| AuthError::Cancelled)??;
        drop(self.listener);

        let resp = http
            .post(TOKEN_URL)
            .form(&[
                ("client_id", client.client_id.as_str()),
                ("client_secret", client.client_secret.expose()),
                ("code", code.expose()),
                ("code_verifier", self.verifier.as_str()),
                ("grant_type", "authorization_code"),
                ("redirect_uri", self.redirect_uri.as_str()),
            ])
            .send()
            .await?;
        let tokens = parse_token_response(resp).await?;
        if tokens.refresh_token.is_none() {
            return Err(AuthError::NoRefreshToken);
        }
        Ok(tokens)
    }
}

async fn wait_for_code(listener: &TcpListener, expected_state: &str) -> Result<Secret, AuthError> {
    loop {
        let (mut stream, _) = listener.accept().await?;
        let Some(target) = read_request_target(&mut stream).await? else {
            continue;
        };
        match parse_redirect(&target, expected_state) {
            Redirect::Ignore => {
                respond(&mut stream, "404 Not Found", "").await;
            }
            Redirect::Code(code) => {
                respond(&mut stream, "200 OK", PAGE_OK).await;
                return Ok(code);
            }
            Redirect::Error(e) => {
                respond(&mut stream, "200 OK", PAGE_FAIL).await;
                return Err(e);
            }
        }
    }
}

#[derive(Debug)]
enum Redirect {
    Ignore,
    Code(Secret),
    Error(AuthError),
}

fn parse_redirect(target: &str, expected_state: &str) -> Redirect {
    let Ok(url) = Url::parse(&format!("http://127.0.0.1{target}")) else {
        return Redirect::Ignore;
    };
    if url.path() != "/" {
        return Redirect::Ignore; // e.g. /favicon.ico
    }
    let mut code = None;
    let mut state = None;
    let mut error = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "code" => code = Some(v.into_owned()),
            "state" => state = Some(v.into_owned()),
            "error" => error = Some(v.into_owned()),
            _ => {}
        }
    }
    if state.as_deref() != Some(expected_state) {
        return if code.is_some() || error.is_some() {
            Redirect::Error(AuthError::StateMismatch)
        } else {
            Redirect::Ignore
        };
    }
    match (code, error) {
        (_, Some(e)) => Redirect::Error(AuthError::Denied(e)),
        (Some(c), None) => Redirect::Code(Secret::new(c)),
        (None, None) => Redirect::Ignore,
    }
}

/// Read just the request line ("GET /path HTTP/1.1") from the browser.
async fn read_request_target(stream: &mut TcpStream) -> Result<Option<String>, AuthError> {
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 1024];
    let read = async {
        while !buf.windows(4).any(|w| w == b"\r\n\r\n") && buf.len() < 16 * 1024 {
            let n = stream.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        Ok::<_, std::io::Error>(())
    };
    if tokio::time::timeout(Duration::from_secs(10), read).await.is_err() {
        return Ok(None);
    }
    let head = String::from_utf8_lossy(&buf);
    let line = head.lines().next().unwrap_or_default();
    let mut parts = line.split_whitespace();
    match (parts.next(), parts.next()) {
        (Some("GET"), Some(target)) => Ok(Some(target.to_string())),
        _ => Ok(None),
    }
}

async fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let resp = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\n\
         Cache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(resp.as_bytes()).await;
    let _ = stream.shutdown().await;
}

const PAGE_OK: &str = "<!doctype html><meta charset=utf-8><title>Pulse Mail</title>\
<body style=\"font:16px system-ui;display:grid;place-items:center;height:90vh\">\
<div><h2>You're signed in to Pulse Mail</h2><p>You can close this tab.</p></div>";
const PAGE_FAIL: &str = "<!doctype html><meta charset=utf-8><title>Pulse Mail</title>\
<body style=\"font:16px system-ui;display:grid;place-items:center;height:90vh\">\
<div><h2>Sign-in didn't finish</h2><p>Close this tab and try again from Pulse Mail.</p></div>";

pub async fn refresh(
    http: &reqwest::Client,
    client: &OAuthClient,
    refresh_token: &Secret,
) -> Result<Tokens, AuthError> {
    let resp = http
        .post(TOKEN_URL)
        .form(&[
            ("client_id", client.client_id.as_str()),
            ("client_secret", client.client_secret.expose()),
            ("refresh_token", refresh_token.expose()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await?;
    parse_token_response(resp).await
}

/// Best-effort revoke on sign-out.
pub async fn revoke(http: &reqwest::Client, token: &Secret) -> Result<(), AuthError> {
    http.post(REVOKE_URL).form(&[("token", token.expose())]).send().await?;
    Ok(())
}

async fn parse_token_response(resp: reqwest::Response) -> Result<Tokens, AuthError> {
    let status = resp.status();
    if !status.is_success() {
        let err = resp
            .json::<TokenErrorResponse>()
            .await
            .map(|e| e.error)
            .unwrap_or_default();
        return Err(if err == "invalid_grant" {
            AuthError::InvalidGrant
        } else {
            AuthError::Protocol(format!("{status} {err}"))
        });
    }
    let t: TokenResponse = resp.json().await?;
    Ok(Tokens {
        access_token: t.access_token,
        // refresh a minute early
        expires_at: Instant::now() + Duration::from_secs(t.expires_in.saturating_sub(60)),
        refresh_token: t.refresh_token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redirect_with_code_and_matching_state() {
        match parse_redirect("/?state=abc&code=4%2F0Ab&scope=x", "abc") {
            Redirect::Code(c) => assert_eq!(c.expose(), "4/0Ab"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn redirect_state_mismatch_is_an_error() {
        assert!(matches!(
            parse_redirect("/?state=evil&code=x", "abc"),
            Redirect::Error(AuthError::StateMismatch)
        ));
    }

    #[test]
    fn redirect_denied() {
        assert!(matches!(
            parse_redirect("/?state=abc&error=access_denied", "abc"),
            Redirect::Error(AuthError::Denied(_))
        ));
    }

    #[test]
    fn unrelated_requests_are_ignored() {
        assert!(matches!(parse_redirect("/favicon.ico", "abc"), Redirect::Ignore));
        assert!(matches!(parse_redirect("/", "abc"), Redirect::Ignore));
    }

    #[tokio::test]
    async fn auth_url_has_pkce_and_loopback() {
        let client =
            super::super::client_config::parse(r#"{"client_id":"1.apps.googleusercontent.com","client_secret":"s"}"#)
                .unwrap();
        let p = begin_login(&client, Some("fixture@example.com")).await.unwrap();
        let url = Url::parse(&p.auth_url).unwrap();
        let q: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["code_challenge"], pkce::challenge_for(&p.verifier));
        assert!(q["redirect_uri"].starts_with("http://127.0.0.1:"));
        assert_eq!(q["access_type"], "offline");
        assert_eq!(q["login_hint"], "fixture@example.com");
        assert_eq!(q["scope"], SCOPE);
        let scopes: std::collections::BTreeSet<_> = q["scope"].split_whitespace().collect();
        assert_eq!(
            scopes,
            ["https://mail.google.com/", "openid", "profile"].into_iter().collect()
        );
        assert!(!p.auth_url.contains("client_secret"));
    }
}
