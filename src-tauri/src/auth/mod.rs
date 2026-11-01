//! Authentication: OAuth client config, PKCE loopback sign-in, refresh-token
//! storage in Windows Credential Manager, and an access-token cache.
//!
//! One [`TokenManager`] per account owns that account's credential in memory;
//! the credential store stays the source of truth (see ADR 0028).

pub mod client_config;
pub mod oauth;
pub mod pkce;
pub mod profile;
pub mod secret;
pub mod token_store;

use client_config::OAuthClient;
use oauth::AuthError;
use secret::Secret;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;
use tokio::sync::Mutex;

/// Hands out a valid access token, refreshing it when needed. Access tokens
/// stay in memory only; the refresh token lives in the credential store.
pub struct TokenManager {
    pub email: String,
    client: OAuthClient,
    http: reqwest::Client,
    inner: Mutex<Inner>,
    /// Set on sign-out, before the store entry is deleted, so a refresh that
    /// is already in flight neither serves nor rewrites the dead credential.
    closed: AtomicBool,
}

struct Inner {
    /// The newest token this process knows of (from sign-in or a rotation;
    /// the store is re-read when Google rejects it).
    refresh_token: Secret,
    /// Access token and the instant it stops being reused.
    access: Option<(Secret, Instant)>,
}

impl TokenManager {
    pub fn new(email: String, client: OAuthClient, http: reqwest::Client, refresh_token: Secret) -> Self {
        Self {
            email,
            client,
            http,
            inner: Mutex::new(Inner {
                refresh_token,
                access: None,
            }),
            closed: AtomicBool::new(false),
        }
    }

    /// The account is signed out: stop serving and never write the store again.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Save and use the credential a sign-in just obtained. Runs under the
    /// same lock as rotation, so this account's store entry has one writer.
    /// Returns false when the account was signed out meanwhile.
    pub async fn adopt(&self, refresh_token: Secret) -> Result<bool, keyring::Error> {
        let mut inner = self.inner.lock().await;
        if self.closed.load(Ordering::SeqCst) {
            return Ok(false);
        }
        self.persist(&refresh_token)?;
        if self.closed.load(Ordering::SeqCst) {
            return Ok(false);
        }
        inner.refresh_token = refresh_token;
        // The old access token belongs to the previous grant.
        inner.access = None;
        Ok(true)
    }

    /// Write the credential, undoing the write if the account was signed out
    /// while it was happening. Sign-out sets `closed` before deleting the
    /// entry, so one of the two deletions always wins.
    fn persist(&self, token: &Secret) -> Result<(), keyring::Error> {
        token_store::save_refresh_token(&self.email, token)?;
        if self.closed.load(Ordering::SeqCst) {
            let _ = token_store::delete_refresh_token(&self.email);
        }
        Ok(())
    }

    pub async fn access_token(&self) -> Result<Secret, AuthError> {
        let mut inner = self.inner.lock().await;
        if self.closed.load(Ordering::SeqCst) {
            return Err(AuthError::InvalidGrant);
        }
        if let Some((tok, exp)) = inner.access.as_ref()
            && Instant::now() < *exp
        {
            return Ok(tok.clone());
        }
        let mut used = inner.refresh_token.clone();
        let tokens = match oauth::refresh(&self.http, &self.client, &used).await {
            Ok(t) => t,
            Err(AuthError::InvalidGrant) => {
                // This process may hold an older token than the credential
                // store (another window, a dev build, a reconnect): retry once
                // with the saved one before declaring the sign-in dead.
                match token_store::load_refresh_token(&self.email) {
                    Ok(Some(saved)) if saved != used => {
                        let t = oauth::refresh(&self.http, &self.client, &saved).await?;
                        used = saved;
                        t
                    }
                    _ => return Err(AuthError::InvalidGrant),
                }
            }
            Err(e) => return Err(e),
        };
        // Signed out while the request was in flight: the store entry is gone,
        // so this token must not be served or written back.
        if self.closed.load(Ordering::SeqCst) {
            return Err(AuthError::InvalidGrant);
        }
        let oauth::Tokens {
            access_token,
            expires_at,
            refresh_token,
            ..
        } = tokens;
        inner.access = Some((access_token.clone(), expires_at));
        // Google can rotate the refresh token on any refresh; keep and store
        // the newest one, otherwise the next refresh fails with invalid_grant.
        inner.refresh_token = match refresh_token.filter(|t| t != &used) {
            Some(fresh) => {
                if let Err(e) = self.persist(&fresh) {
                    tracing::warn!(error = %e, "could not save the renewed Google sign-in");
                }
                fresh
            }
            None => used,
        };
        Ok(access_token)
    }

    /// Drop the cached token (e.g. after a 401) so the next call refreshes.
    pub async fn invalidate(&self) {
        self.inner.lock().await.access = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager(email: &str) -> TokenManager {
        let client = client_config::parse(r#"{"client_id":"1.apps.googleusercontent.com","client_secret":"s"}"#)
            .expect("fixture client");
        TokenManager::new(
            email.to_string(),
            client,
            reqwest::Client::new(),
            Secret::new("fixture-refresh-token"),
        )
    }

    #[tokio::test]
    async fn closed_manager_never_serves_a_token() {
        // reqwest's rustls client needs a process-wide provider.
        let _ = rustls::crypto::ring::default_provider().install_default();
        let m = manager("nobody@example.com");
        m.close();
        // No network call and no credential-store access may happen here.
        assert!(matches!(m.access_token().await, Err(AuthError::InvalidGrant)));
    }

    #[tokio::test]
    async fn closed_manager_refuses_a_new_credential() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let m = manager("nobody@example.com");
        m.close();
        // No credential-store write and no token change may happen here.
        assert!(!m.adopt(Secret::new("new-refresh")).await.expect("no store access"));
        let inner = m.inner.lock().await;
        assert_eq!(inner.refresh_token, Secret::new("fixture-refresh-token"));
        assert!(inner.access.is_none());
    }
}
