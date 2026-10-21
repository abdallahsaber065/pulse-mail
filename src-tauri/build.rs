/// Every IPC command the web UI may call. Each one gets an `allow-<name>`
/// permission that `capabilities/default.json` must grant explicitly; a
/// command missing from either list is unreachable from the UI. Kept in sync
/// with `generate_handler!` by `security::tests::command_allowlist_in_sync`.
const COMMANDS: &[&str] = &[
    "get_status",
    "sign_in",
    "sign_out",
    "list_labels",
    "list_threads",
    "search",
    "search_server",
    "set_account_settings",
    "account_storage",
    "get_thread",
    "sync_now",
    "take_pending_open",
    "copy_text",
    "open_external",
    "set_destroy_on_close",
    "set_autostart",
    "load_thread",
    "modify_thread",
    "get_activity",
    "set_notifications",
    "account_estimates",
    "finish_onboarding",
    "set_sync_window",
    "pause_history",
    "resume_history",
    "compose_context",
    "save_draft",
    "delete_draft",
    "send_message",
    "cancel_send",
    "stage_attachment",
    "unstage_attachment",
    "render_message",
    "set_image_sender",
    "set_pref",
    "save_attachment",
];

fn main() {
    let mut attrs = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));

    // Tauri embeds its Common-Controls v6 manifest into the app binary only.
    // Test harnesses link the same code (TaskDialogIndirect & co.) and then die
    // with STATUS_ENTRYPOINT_NOT_FOUND (0xc0000139). Embed our own manifest
    // into every linked target (app, tests, examples) instead.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os == "windows" && target_env == "msvc" {
        attrs = attrs.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        println!("cargo:rerun-if-changed=windows-app-manifest.xml");
        // Copy to OUT_DIR: the repo path contains spaces, which link args don't survive.
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
        let manifest = out.join("app.manifest");
        std::fs::copy("windows-app-manifest.xml", &manifest).expect("copy manifest");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }

    tauri_build::try_build(attrs).expect("tauri build script failed");
}
