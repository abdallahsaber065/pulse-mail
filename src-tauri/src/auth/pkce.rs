//! RFC 7636 PKCE (S256) and CSRF state generation.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

/// 32 random bytes -> 43-char base64url verifier (RFC 7636 §4.1).
pub fn generate() -> Pkce {
    let verifier = random_urlsafe(32);
    let challenge = challenge_for(&verifier);
    Pkce { verifier, challenge }
}

pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

pub fn random_urlsafe(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    getrandom::fill(&mut buf).expect("OS random number generator unavailable");
    URL_SAFE_NO_PAD.encode(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_appendix_b_vector() {
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn verifier_shape_and_uniqueness() {
        let a = generate();
        let b = generate();
        assert_eq!(a.verifier.len(), 43);
        assert!(
            a.verifier
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
        assert_ne!(a.verifier, b.verifier);
        assert_eq!(a.challenge, challenge_for(&a.verifier));
    }
}
