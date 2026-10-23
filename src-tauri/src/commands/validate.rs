//! Input checks for IPC commands. The web UI is trusted code, but a bug or a
//! compromised renderer must not be able to smuggle odd values into Gmail
//! URLs, SQL (always parameterized anyway) or the file system.

use super::CmdError;

/// Gmail message/thread/draft ids: short alphanumerics (hex in practice).
pub fn gmail_id(id: &str) -> Result<(), CmdError> {
    if !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') {
        Ok(())
    } else {
        Err(CmdError::new("That message isn't available."))
    }
}

/// Label ids: system names (INBOX, CATEGORY_SOCIAL) or `Label_123`.
pub fn label_id(id: &str) -> Result<(), CmdError> {
    if !id.is_empty() && id.len() <= 64 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-') {
        Ok(())
    } else {
        Err(CmdError::new("Unknown label."))
    }
}

/// Free text with a length cap (search queries, etc.).
pub fn text(s: &str, max: usize) -> Result<(), CmdError> {
    if s.len() <= max && !s.contains('\0') {
        Ok(())
    } else {
        Err(CmdError::new("That text is too long."))
    }
}

/// A single email address, as typed in a recipient field.
pub fn email(s: &str) -> Result<(), CmdError> {
    let s = s.trim();
    let ok = s.len() <= 254
        && !s
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || "<>,;\"()[]\\".contains(c))
        && s.split_once('@')
            .is_some_and(|(l, d)| !l.is_empty() && d.contains('.') && !d.starts_with('.') && !d.ends_with('.'));
    if ok {
        Ok(())
    } else {
        Err(CmdError::new(format!(
            "\"{}\" isn't a valid email address.",
            s.chars().take(80).collect::<String>()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert!(gmail_id("18f3a9c2b1d0e4f5").is_ok());
        for bad in ["", "../etc", "a/b", "a?b=1", "x y", &"a".repeat(65)] {
            assert!(gmail_id(bad).is_err(), "{bad}");
        }
        assert!(label_id("Label_12").is_ok() && label_id("CATEGORY_SOCIAL").is_ok());
        assert!(label_id("INBOX OR 1=1").is_err());
    }

    #[test]
    fn emails() {
        assert!(email("abdallah@example.com").is_ok());
        assert!(email("a.b+tag@sub.example.co").is_ok());
        for bad in [
            "",
            "no-at",
            "a@b",
            "a@.com",
            "a b@x.com",
            "a@x.com\r\nBcc: v@x.com",
            "<a@x.com>",
            "a@x.com,b@x.com",
        ] {
            assert!(email(bad).is_err(), "{bad:?}");
        }
    }
}
