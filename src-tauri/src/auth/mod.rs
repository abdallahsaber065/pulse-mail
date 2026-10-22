//! Authentication: OAuth client config, PKCE loopback sign-in, refresh-token
//! storage in Windows Credential Manager, and an access-token cache.

pub mod client_config;
pub mod oauth;
pub mod pkce;
pub mod profile;
pub mod secret;
pub mod token_store;

use client_config::OAuthClient;
use oauth::AuthError;
use secret::Secret;
use std::time::Instant;
use tokio::sync::Mutex;

/// Hands out a valid access token, refreshing it when needed. Access tokens
/// stay in memory only.
pub struct TokenManager {
    pub email: String,
    client: OAuthClient,
    http: reqwest::Client,
    refresh_token: Secret,
    cached: Mutex<Option<(Secret, Instant)>>,
}

impl TokenManager {
    pub fn new(email: String, client: OAuthClient, http: reqwest::Client, refresh_token: Secret) -> Self {
        Self {
            email,
            client,
            http,
            refresh_token,
            cached: Mutex::new(None),
        }
    }

    pub async fn access_token(&self) -> Result<Secret, AuthError> {
        let mut cached = self.cached.lock().await;
        if let Some((tok, exp)) = cached.as_ref()
            && Instant::now() < *exp
        {
            return Ok(tok.clone());
        }
        let t = oauth::refresh(&self.http, &self.client, &self.refresh_token).await?;
        *cached = Some((t.access_token.clone(), t.expires_at));
        Ok(t.access_token)
    }

    /// Drop the cached token (e.g. after a 401) so the next call refreshes.
    pub async fn invalidate(&self) {
        *self.cached.lock().await = None;
    }
}
