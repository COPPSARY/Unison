//! Unison app entry: builder, plugins and (from T-05 on) command registration.

pub fn run() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!());

    if let Err(err) = result {
        // Logging via `tracing` arrives in T-05.
        eprintln!("unison failed to start: {err}");
        std::process::exit(1);
    }
}
