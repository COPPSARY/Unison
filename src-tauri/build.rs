fn main() {
    // Tauri needs Common Controls v6 on Windows. tauri-build only embeds that manifest into the
    // app binary, so `cargo test` binaries crash with STATUS_ENTRYPOINT_NOT_FOUND. Instead, ask
    // the MSVC linker to embed the dependency into every target (app, tests, cdylib).
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let msvc = target_os == "windows" && target_env == "msvc";

    let mut attrs = tauri_build::Attributes::new();
    if msvc {
        attrs = attrs.windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }

    if let Err(err) = tauri_build::try_build(attrs) {
        // build scripts report failure by panicking; this is the only allowed panic path.
        panic!("tauri-build failed: {err:#}");
    }
}
