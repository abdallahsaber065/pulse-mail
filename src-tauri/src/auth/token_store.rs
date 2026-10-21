//! Refresh tokens live only in Windows Credential Manager (via `keyring`).
//! They are never written to SQLite, config files or logs.

use super::secret::Secret;

const SERVICE: &str = "com.abdallah.pulsemail";

fn entry(email: &str) -> keyring::Result<keyring::Entry> {
    keyring::Entry::new(SERVICE, &format!("refresh-token:{email}"))
}

pub fn save_refresh_token(email: &str, token: &Secret) -> keyring::Result<()> {
    entry(email)?.set_password(token.expose())
}

pub fn load_refresh_token(email: &str) -> keyring::Result<Option<Secret>> {
    match entry(email)?.get_password() {
        Ok(t) => Ok(Some(Secret::new(t))),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e),
    }
}

pub fn delete_refresh_token(email: &str) -> keyring::Result<()> {
    match entry(email)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e),
    }
}
