pub mod auth;
pub mod db;
pub mod extensions;
pub mod local;
pub mod logging;
pub mod matching;
pub mod playback;
pub mod window_material;

/// Current time in Unix seconds (0 if the clock is before 1970).
pub fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
