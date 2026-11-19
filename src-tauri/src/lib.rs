//! Unison app entry: builder, plugins, state and command registration.

mod bindings;
mod commands;
pub mod core;
pub mod error;
pub mod model;
pub mod model_player;
pub mod providers;
mod state;

use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_window_state::StateFlags;

use crate::core::window_material::{self, Material};
use std::sync::Arc;

use crate::core::auth::keychain::Keychain;
use crate::state::{AppState, LogGuard};

/// Builds the main window in code so the backdrop material can depend on the Windows build.
fn create_main_window(app: &tauri::App, saved: model::WindowMaterial) -> tauri::Result<()> {
    let material = window_material::resolve(Material::requested(saved), window_material::windows_build());

    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("Unison")
        .inner_size(1280.0, 800.0)
        .min_inner_size(960.0, 600.0)
        // Custom title bar (T-04); Windows 11 still draws the rounded frame and shadow.
        .decorations(false)
        // https://tauri.localhost in production: YouTube embeds need a real https origin and
        // referrer (Universal Player addendum §0, UP-03).
        .use_https_scheme(true)
        .shadow(true)
        .initialization_script(window_material::init_script(material));

    if let Some(effects) = material.effects() {
        builder = builder.transparent(true).effects(effects);
    }

    builder.build()?;
    Ok(())
}

/// Logging, database and state. Errors here abort startup with a logged reason.
/// Returns the saved window material so the window can be created with it.
fn init_core(app: &tauri::App) -> Result<model::WindowMaterial, Box<dyn std::error::Error>> {
    let paths = app.path();
    let guard = core::logging::init(&paths.app_log_dir()?)?;
    app.manage(LogGuard(guard));
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "starting unison");

    // UNISON_DB points at another database (e.g. the `pnpm seed` demo DB) for UI checks.
    let db_path = match std::env::var_os("UNISON_DB") {
        Some(custom) => std::path::PathBuf::from(custom),
        None => paths.app_data_dir()?.join(core::db::DB_FILE),
    };
    let pool = tauri::async_runtime::block_on(core::db::open(&db_path)).map_err(|e| {
        tracing::error!(error = %e, "database failed to open");
        e
    })?;
    let settings = tauri::async_runtime::block_on(core::db::settings::load(&pool)).unwrap_or_default();
    let native = core::playback::native::NativeEngine::spawn(
        settings.player.output_device.clone(),
        settings.player.volume,
    )?;
    app.manage(AppState::new(pool, state::http_client()?, Arc::new(Keychain), Arc::new(native)));
    Ok(settings.window_material)
}

pub fn run() {
    let specta = bindings::builder();

    #[cfg(debug_assertions)]
    if let Err(err) = bindings::export(&specta, bindings::BINDINGS_PATH) {
        eprintln!("failed to export TypeScript bindings: {err}");
    }

    let result = tauri::Builder::default()
        // Must be first: a second launch hands over to the running instance and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
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
            let material = init_core(app)?;
            create_main_window(app, material)?;
            core::sync::scheduler::start(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(err) = result {
        tracing::error!(error = %err, "unison exited with an error");
        eprintln!("unison failed to start: {err}");
        std::process::exit(1);
    }
}
