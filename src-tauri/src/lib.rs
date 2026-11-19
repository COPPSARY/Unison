//! Unison app entry: builder, plugins, state and command registration.

mod bindings;
mod commands;
mod core;
mod error;
mod model;
mod state;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_window_state::StateFlags;

use crate::core::window_material::{self, Material};
use crate::state::{AppState, LogGuard};

/// Builds the main window in code so the backdrop material can depend on the Windows build.
fn create_main_window(app: &tauri::App) -> tauri::Result<()> {
    let material = window_material::resolve(Material::requested(), window_material::windows_build());

    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("Unison")
        .inner_size(1280.0, 800.0)
        .min_inner_size(960.0, 600.0)
        // Custom title bar (T-04); Windows 11 still draws the rounded frame and shadow.
        .decorations(false)
        .shadow(true)
        .initialization_script(window_material::init_script(material));

    if let Some(effects) = material.effects() {
        builder = builder.transparent(true).effects(effects);
    }

    builder.build()?;
    Ok(())
}

/// Logging, database and state. Errors here abort startup with a logged reason.
fn init_core(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let paths = app.path();
    let guard = core::logging::init(&paths.app_log_dir()?)?;
    app.manage(LogGuard(guard));
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting unison");

    let db_path = paths.app_data_dir()?.join(core::db::DB_FILE);
    let pool = tauri::async_runtime::block_on(core::db::open(&db_path)).map_err(|e| {
        tracing::error!(error = %e, "database failed to open");
        e
    })?;
    app.manage(AppState { db: pool });
    Ok(())
}

pub fn run() {
    let specta = bindings::builder();

    #[cfg(debug_assertions)]
    if let Err(err) = bindings::export(&specta, bindings::BINDINGS_PATH) {
        eprintln!("failed to export TypeScript bindings: {err}");
    }

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Restores size/position/maximised for windows created later in setup().
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .build(),
        )
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            specta.mount_events(app);
            init_core(app)?;
            create_main_window(app)?;
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(err) = result {
        tracing::error!(error = %err, "unison exited with an error");
        eprintln!("unison failed to start: {err}");
        std::process::exit(1);
    }
}
