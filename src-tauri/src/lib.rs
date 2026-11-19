//! Unison app entry: builder, plugins and (from T-05 on) command registration.

mod core;

use crate::core::window_material::{self, Material};
use tauri::{WebviewUrl, WebviewWindowBuilder};

/// Builds the main window in code so the backdrop material can depend on the Windows build.
fn create_main_window(app: &tauri::App) -> tauri::Result<()> {
    let material = window_material::resolve(Material::requested(), window_material::windows_build());

    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("Unison")
        .inner_size(1280.0, 800.0)
        .min_inner_size(960.0, 600.0)
        .initialization_script(window_material::init_script(material));

    if let Some(effects) = material.effects() {
        builder = builder.transparent(true).effects(effects);
    }

    builder.build()?;
    Ok(())
}

pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            create_main_window(app)?;
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(err) = result {
        // Logging via `tracing` arrives in T-05.
        eprintln!("unison failed to start: {err}");
        std::process::exit(1);
    }
}
