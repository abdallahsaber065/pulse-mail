//! Static security invariants checked at test time.
//!
//! - Every IPC command is in the build-script allowlist and granted by the
//!   capability file (and nothing else is granted).
//! - The capability grants no window/webview/path/plugin APIs.
//! - Logging never formats tokens, bodies or verification codes.
//! - Release builds have no devtools (the `devtools` feature stays off).

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    const LIB: &str = include_str!("lib.rs");
    const BUILD: &str = include_str!("../build.rs");
    const CAPS: &str = include_str!("../capabilities/default.json");
    const CARGO: &str = include_str!("../Cargo.toml");

    fn handler_commands() -> BTreeSet<String> {
        let start = LIB.find("generate_handler![").expect("handler") + "generate_handler![".len();
        let end = start + LIB[start..].find(']').expect("end");
        LIB[start..end]
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.rsplit("::").next().unwrap().to_string())
            .collect()
    }

    fn build_commands() -> BTreeSet<String> {
        let start = BUILD.find("const COMMANDS").expect("COMMANDS");
        let body = &BUILD[start..start + BUILD[start..].find("];").expect("end")];
        body.split('"').skip(1).step_by(2).map(str::to_string).collect()
    }

    #[test]
    fn command_allowlist_in_sync() {
        let handler = handler_commands();
        assert!(handler.len() > 10);
        assert_eq!(handler, build_commands(), "build.rs COMMANDS != generate_handler!");
        let caps: serde_json::Value = serde_json::from_str(CAPS).unwrap();
        let granted: BTreeSet<String> = caps["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_str().expect("plain permission ids only").to_string())
            .collect();
        let mut expected: BTreeSet<String> = handler
            .iter()
            .map(|c| format!("allow-{}", c.replace('_', "-")))
            .collect();
        expected.insert("core:event:allow-listen".into());
        expected.insert("core:event:allow-unlisten".into());
        assert_eq!(granted, expected, "capability grants differ from the command list");
    }

    #[test]
    fn no_devtools_in_release() {
        let tauri_line = CARGO.lines().find(|l| l.starts_with("tauri =")).unwrap();
        assert!(!tauri_line.contains("devtools"), "devtools feature must stay off");
    }

    #[test]
    fn logging_never_formats_secrets() {
        let sources = [
            include_str!("lib.rs"),
            include_str!("commands/mod.rs"),
            include_str!("commands/mail.rs"),
            include_str!("sync/mod.rs"),
            include_str!("sync/mime.rs"),
            include_str!("sync/imap_idle.rs"),
            include_str!("auth/mod.rs"),
            include_str!("auth/oauth.rs"),
            include_str!("auth/profile.rs"),
            include_str!("notify/mod.rs"),
            include_str!("render/mod.rs"),
            include_str!("render/proxy.rs"),
        ];
        const BAD: &[&str] = &[
            "access_token",
            "refresh_token",
            "expose(",
            ".body",
            "raw",
            "html",
            "otp_code",
            "code =",
            "snippet",
            "password",
            "text =",
        ];
        for src in sources {
            for (i, chunk) in src.split("tracing::").enumerate().skip(1) {
                // The macro call up to its closing `);`.
                let call = &chunk[..chunk.find(");").unwrap_or(chunk.len()).min(400)];
                if !["info!", "warn!", "debug!", "error!", "trace!"]
                    .iter()
                    .any(|m| call.starts_with(m))
                {
                    continue;
                }
                for bad in BAD {
                    assert!(!call.contains(bad), "log call #{i} formats `{bad}`: tracing::{call}");
                }
            }
        }
    }
}
