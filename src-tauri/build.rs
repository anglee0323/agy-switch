fn main() {
    // Unit-test executables also import Common Controls v6 through Tauri's menu
    // stack. Own the manifest at the linker boundary for binaries/tests/examples;
    // Tauri continues to supply the icon and VERSION resource, without a second
    // manifest resource. See tauri-apps/tauri#13948 and its official API example.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(
            tauri_build::WindowsAttributes::new_without_app_manifest(),
        )).expect("failed to build Tauri resources");
        let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
            .join("../scripts/fixtures/windows-desktop.manifest");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    } else {
        tauri_build::build();
    }
}
