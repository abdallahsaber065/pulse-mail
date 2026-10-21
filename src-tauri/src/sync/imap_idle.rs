//! IMAP IDLE "doorbell". Holds one TLS connection to imap.gmail.com in IDLE
//! on INBOX and rings `wake` the moment Gmail reports a change. It never
//! downloads mail itself; the REST history sync does that.

use super::status::StatusCell;
use crate::auth::TokenManager;
use async_imap::extensions::idle::IdleResponse;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::Notify;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

const HOST: &str = "imap.gmail.com";
const PORT: u16 = 993;
/// RFC 2177: re-issue IDLE before 29 minutes; Gmail and NAT boxes may drop
/// silent connections, so stay well under.
const IDLE_REFRESH: Duration = Duration::from_secs(20 * 60);
const MAX_BACKOFF: Duration = Duration::from_secs(60);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

type Session = async_imap::Session<TlsStream<TcpStream>>;

struct XOAuth2 {
    user: String,
    access_token: String,
}

impl async_imap::Authenticator for &XOAuth2 {
    type Response = String;
    fn process(&mut self, _challenge: &[u8]) -> Self::Response {
        format!("user={}\x01auth=Bearer {}\x01\x01", self.user, self.access_token)
    }
}

#[derive(Debug, thiserror::Error)]
enum ImapError {
    #[error("imap: {0}")]
    Imap(#[from] async_imap::error::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("auth: {0}")]
    Auth(#[from] crate::auth::oauth::AuthError),
    #[error("timeout")]
    Timeout,
    #[error("server closed connection")]
    Closed,
}

/// Runs forever (until the task is aborted), reconnecting with backoff.
pub async fn run(tokens: Arc<TokenManager>, wake: Arc<Notify>, status: Arc<StatusCell>) {
    use futures::FutureExt;
    let mut backoff = Duration::from_secs(1);
    loop {
        let attempt = std::panic::AssertUnwindSafe(watch_once(&tokens, &wake, &status))
            .catch_unwind()
            .await;
        let was_live = status.imap_connected();
        let e = match attempt {
            Ok(Ok(())) => {
                backoff = Duration::from_secs(1);
                continue;
            }
            Ok(Err(e)) => e,
            Err(p) => {
                tracing::error!(panic = %super::panic_text(&p), "imap task panicked");
                ImapError::Closed
            }
        };
        status.set_imap_connected(false);
        tracing::warn!(error = %e, retry_in = ?backoff, "imap idle connection lost");
        if was_live {
            status.log(
                "warn",
                "Live connection dropped. Reconnecting; checking every minute meanwhile.",
            );
        }
        if matches!(e, ImapError::Imap(async_imap::error::Error::No(_))) {
            // Most likely an expired access token: force a refresh.
            tokens.invalidate().await;
        }
        if let ImapError::Auth(crate::auth::oauth::AuthError::InvalidGrant) = e {
            // Sign-in revoked: the engine reports it; check rarely.
            tokio::time::sleep(Duration::from_secs(15 * 60)).await;
            continue;
        }
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

async fn watch_once(tokens: &TokenManager, wake: &Notify, status: &StatusCell) -> Result<(), ImapError> {
    let token = tokens.access_token().await?;
    let mut session = tokio::time::timeout(CONNECT_TIMEOUT, connect(&tokens.email, token.expose()))
        .await
        .map_err(|_| ImapError::Timeout)??;
    session.select("INBOX").await?;
    status.set_imap_connected(true);
    tracing::info!("imap idle connected");
    status.log("success", "Live: Gmail will push new mail instantly");
    // Catch up on anything that arrived while we were disconnected.
    wake.notify_one();

    loop {
        let mut idle = session.idle();
        idle.init().await?;
        let (wait, stop) = idle.wait_with_timeout(IDLE_REFRESH);
        let res = wait.await?;
        drop(stop); // keep the stop source alive until the wait finishes
        session = idle.done().await?;
        match res {
            IdleResponse::NewData(_) => {
                tracing::debug!("imap idle: new data");
                wake.notify_one();
            }
            IdleResponse::Timeout => {
                // Re-IDLE; a NOOP proves the connection is still alive.
                session.noop().await?;
            }
            IdleResponse::ManualInterrupt => return Err(ImapError::Closed),
        }
    }
}

async fn connect(user: &str, access_token: &str) -> Result<Session, ImapError> {
    let tcp = TcpStream::connect((HOST, PORT)).await?;
    tcp.set_nodelay(true)?;
    let connector = TlsConnector::from(Arc::new(tls_config()));
    let server_name = rustls::pki_types::ServerName::try_from(HOST).expect("valid host");
    let tls = connector.connect(server_name, tcp).await?;

    let mut client = async_imap::Client::new(tls);
    // Gmail sends a greeting first; authenticating before reading it hangs.
    client.read_response().await?.ok_or(ImapError::Closed)?;
    let auth = XOAuth2 {
        user: user.to_string(),
        access_token: access_token.to_string(),
    };
    let session = client.authenticate("XOAUTH2", &auth).await.map_err(|(e, _)| e)?;
    Ok(session)
}

fn tls_config() -> rustls::ClientConfig {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth()
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_imap::Authenticator;

    #[test]
    fn xoauth2_sasl_string() {
        let a = XOAuth2 {
            user: "me@gmail.com".into(),
            access_token: "tok".into(),
        };
        let mut r = &a;
        assert_eq!(r.process(b""), "user=me@gmail.com\x01auth=Bearer tok\x01\x01");
    }
}
