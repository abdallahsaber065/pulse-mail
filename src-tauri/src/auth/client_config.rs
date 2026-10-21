//! Loads the user's own Google OAuth "Desktop app" client from a local file
//! that is never committed. See docs/SETUP-GOOGLE-OAUTH.md.
//!
//! Accepted formats:
//! - the JSON Google Cloud lets you download (`{"installed": {...}}`)
//! - a flat `{"client_id": "...", "client_secret": "..."}`

use super::secret::Secret;
use serde::Deserialize;
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "google-oauth-client.json";
/// Optional override for the file location.
pub const ENV_OVERRIDE: &str = "PULSE_MAIL_OAUTH_CLIENT";

#[derive(Debug, Clone, Deserialize)]
pub struct OAuthClient {
    pub client_id: String,
    /// For Desktop clients Google treats this as non-confidential, but we
    /// still keep it out of the repo and out of logs.
    pub client_secret: Secret,
}

#[derive(Deserialize)]
struct Wrapped {
    installed: OAuthClient,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("OAuth client file not found at {0}")]
    Missing(PathBuf),
    #[error("OAuth client file is not valid: {0}")]
    Invalid(String),
}

/// Where the client file is expected: `$PULSE_MAIL_OAUTH_CLIENT` or
/// `<app config dir>\google-oauth-client.json`.
pub fn resolve_path(app_config_dir: &Path) -> PathBuf {
    match std::env::var_os(ENV_OVERRIDE) {
        Some(p) if !p.is_empty() => PathBuf::from(p),
        _ => app_config_dir.join(FILE_NAME),
    }
}

pub fn load(path: &Path) -> Result<OAuthClient, ConfigError> {
    let raw = std::fs::read_to_string(path).map_err(|_| ConfigError::Missing(path.to_path_buf()))?;
    parse(&raw)
}

pub fn parse(raw: &str) -> Result<OAuthClient, ConfigError> {
    let client = serde_json::from_str::<Wrapped>(raw)
        .map(|w| w.installed)
        .or_else(|_| serde_json::from_str::<OAuthClient>(raw))
        // Do not echo serde's message: it can quote file content.
        .map_err(|_| ConfigError::Invalid("expected an 'installed' Desktop-app client".into()))?;
    if !client.client_id.ends_with(".apps.googleusercontent.com") {
        return Err(ConfigError::Invalid(
            "client_id does not look like a Google OAuth client".into(),
        ));
    }
    if client.client_secret.expose().is_empty() {
        return Err(ConfigError::Invalid("client_secret is empty".into()));
    }
    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_google_download_format() {
        let raw = r#"{"installed":{"client_id":"123-abc.apps.googleusercontent.com","project_id":"p","auth_uri":"https://accounts.google.com/o/oauth2/auth","token_uri":"https://oauth2.googleapis.com/token","client_secret":"GOCSPX-x","redirect_uris":["http://localhost"]}}"#;
        let c = parse(raw).unwrap();
        assert_eq!(c.client_id, "123-abc.apps.googleusercontent.com");
        assert_eq!(c.client_secret.expose(), "GOCSPX-x");
    }

    #[test]
    fn parses_flat_format() {
        let c = parse(r#"{"client_id":"1.apps.googleusercontent.com","client_secret":"s"}"#).unwrap();
        assert_eq!(c.client_id, "1.apps.googleusercontent.com");
    }

    #[test]
    fn rejects_web_clients_and_garbage() {
        assert!(parse(r#"{"web":{"client_id":"1.apps.googleusercontent.com","client_secret":"s"}}"#).is_err());
        assert!(parse("not json").is_err());
        assert!(parse(r#"{"client_id":"nope","client_secret":"s"}"#).is_err());
    }

    #[test]
    fn missing_file_is_reported() {
        assert!(matches!(
            load(Path::new("Z:/definitely/missing.json")),
            Err(ConfigError::Missing(_))
        ));
    }
}
