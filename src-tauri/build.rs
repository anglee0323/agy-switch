fn main() {
    tauri_build::build();
    // Tauri embeds the Common Controls manifest in binaries, not examples.
    // The opt-in Windows lifecycle example also links native menu controls.
    if std::env::var_os("CARGO_FEATURE_NATIVE_GUI_TEST").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("../scripts/fixtures/windows-desktop.manifest");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg-examples=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-examples=/MANIFESTINPUT:{}", manifest.display());
    }
}
