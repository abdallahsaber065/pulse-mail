fn main() {
    let mut attrs = tauri_build::Attributes::new();

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
