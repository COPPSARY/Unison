//! Unison app entry: builder, plugins, state and command registration.

mod bindings;
mod commands;
pub mod core;
pub mod error;
pub mod model;
pub mod model_player;
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
        // https://tauri.localhost in production (keeps the webview origin stable for storage).
        .use_https_scheme(true)
        .shadow(true)
        .initialization_script(window_material::init_script(material));

    if let Some(effects) = material.effects() {
        builder = builder.transparent(true).effects(effects);
    }

    let window = builder.build()?;
    // Files dropped anywhere on the window are imported (UP-11).
    let handle = app.handle().clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::DragDrop(drop) = event {
            core::local::jobs::on_drag_drop(&handle, drop);
        }
    });
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
    // Extension streams buffer in a temporary file that is deleted when the track unloads.
    match paths.app_cache_dir() {
        Ok(dir) => core::playback::http_source::set_buffer_dir(dir.join("stream-buffer")),
        Err(e) => tracing::warn!(error = %e, "no cache folder; streams buffer in memory"),
    }
    let native = core::playback::native::NativeEngine::spawn(
        settings.player.output_device.clone(),
        settings.player.volume,
    )?;
    // Extension artwork shares the artwork:// cache (served below); script extensions keep
    // their credentials in the keychain.
    let extensions = core::extensions::ExtensionManager::new(
        pool.clone(),
        paths.app_data_dir().ok().map(|d| d.join("artwork")),
        Arc::new(Keychain),
    )?;
    forget_streaming_accounts(&Keychain);
    app.manage(AppState::new(pool, Arc::new(native), Arc::new(extensions), settings.player.volume));
    Ok(settings.window_material)
}

/// Spotify and YouTube were removed: drop their client id and tokens from the keychain.
fn forget_streaming_accounts(store: &dyn core::auth::keychain::SecretStore) {
    for key in ["spotify.client_id", "spotify.tokens", "youtube.tokens"] {
        if let Err(e) = store.delete(key) {
            tracing::debug!(%key, error = %e, "keychain cleanup skipped");
        }
    }
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
        // Native Add files / Add folder dialogs, opened from Rust only (no JS permissions).
        .plugin(tauri_plugin_dialog::init())
        // Saved covers of local files (artwork://localhost/<hash>.jpg); nothing else is served.
        .register_uri_scheme_protocol(core::local::artwork::SCHEME, |ctx, request| {
            match ctx.app_handle().path().app_data_dir() {
                Ok(dir) => core::local::artwork::serve(&dir.join("artwork"), request.uri().path()),
                Err(_) => tauri::http::Response::builder()
                    .status(tauri::http::StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .unwrap_or_default(),
            }
        })
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
            core::playback::service::start(app.handle());
            core::local::jobs::init(app.handle());
            Ok(())
        })
        .run(tauri::generate_context!());

    if let Err(err) = result {
        tracing::error!(error = %err, "unison exited with an error");
        eprintln!("unison failed to start: {err}");
        std::process::exit(1);
    }
}
