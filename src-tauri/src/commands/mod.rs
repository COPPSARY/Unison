//! Thin `#[tauri::command]` functions, one file per area. No provider logic here.

pub mod accounts;
pub mod app;
pub mod library;
pub mod local;
pub mod player;
pub mod playlists;
pub mod settings;
pub mod sync;
pub mod youtube;
