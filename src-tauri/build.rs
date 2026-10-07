fn main() {
    tauri_build::build();
    // Tauri embeds the Common Controls manifest in binaries, not examples.
    // The opt-in Windows lifecycle example also links native menu controls.
    if std::env::var_os("CARGO_FEATURE_NATIVE_GUI_TEST").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        println!("cargo:rustc-link-arg-examples=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-examples=/MANIFESTDEPENDENCY:\"type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'\"");
    }
}
