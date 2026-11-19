//! YouTube playback bridge (addendum §0/§1, UP-03). The official IFrame player lives in the
//! webview (`YouTubeHost`); Rust sends it commands through `player://youtube-command` and the
//! UI reports back. No audio is ever extracted; the player stays visible (≥ 200×200 px).

use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;
use tokio::sync::oneshot;

use crate::error::{AppError, AppResult, ErrorCode};

/// Known embeddable video used by Settings → YouTube player → Test playback.
/// ("Me at the zoo", the first YouTube upload; embedding allowed.)
pub const TEST_VIDEO_ID: &str = "jNQXAC9IVRw";
pub const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Commands for the webview's YouTubeHost (spec §5 `player://youtube-command`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, Event)]
#[serde(tag = "type", rename_all = "camelCase")]
#[tauri_specta(event_name = "player://youtube-command")]
pub enum YoutubeCommand {
    Load {
        #[serde(rename = "videoId")]
        video_id: String,
        #[serde(rename = "startMs")]
        start_ms: u32,
    },
    Play,
    Pause,
    Seek {
        #[serde(rename = "positionMs")]
        position_ms: u32,
    },
    Stop,
    /// Play the test video and report through `youtube_test_report`.
    Test {
        #[serde(rename = "videoId")]
        video_id: String,
    },
}

/// Maps an IFrame player `onError` code to an app error.
/// 2 = bad parameter, 5 = HTML5 player error, 100 = not found/private,
/// 101 / 150 = owner disallows embedding, 152 / 153 = embed configuration (e.g. missing referrer).
pub fn embed_error(code: u16) -> AppError {
    match code {
        100 => AppError::not_found("youtube video not found or private"),
        _ => AppError::new(ErrorCode::YoutubeEmbedBlocked { code }, format!("youtube embed error {code}")),
    }
}

/// Holds the pending Test playback request until the UI reports back.
#[derive(Default)]
pub struct YoutubeTestHub {
    pending: Mutex<Option<oneshot::Sender<Result<(), u16>>>>,
}

impl YoutubeTestHub {
    /// Registers a new test, replacing (and failing) any older one.
    pub fn begin(&self) -> oneshot::Receiver<Result<(), u16>> {
        let (tx, rx) = oneshot::channel();
        if let Ok(mut slot) = self.pending.lock() {
            if let Some(old) = slot.replace(tx) {
                let _ = old.send(Err(0));
            }
        }
        rx
    }

    /// Called by `youtube_test_report`.
    pub fn report(&self, result: Result<(), u16>) {
        if let Ok(mut slot) = self.pending.lock() {
            if let Some(tx) = slot.take() {
                let _ = tx.send(result);
            }
        }
    }

    /// Waits for the UI's report. Code 0 = no answer in time (player never loaded).
    pub async fn wait(rx: oneshot::Receiver<Result<(), u16>>, timeout: Duration) -> AppResult<()> {
        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(code))) => Err(embed_error(code)),
            Ok(Err(_)) | Err(_) => Err(AppError::new(
                ErrorCode::YoutubeEmbedBlocked { code: 0 },
                "youtube test player did not respond",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_iframe_error_codes() {
        assert_eq!(embed_error(153).code, ErrorCode::YoutubeEmbedBlocked { code: 153 });
        assert_eq!(embed_error(150).code, ErrorCode::YoutubeEmbedBlocked { code: 150 });
        assert_eq!(embed_error(101).code, ErrorCode::YoutubeEmbedBlocked { code: 101 });
        assert_eq!(embed_error(100).code, ErrorCode::NotFound);
    }

    #[test]
    fn command_serialises_for_the_ui() {
        let v = serde_json::to_value(YoutubeCommand::Load { video_id: "x".into(), start_ms: 5 }).unwrap();
        assert_eq!(v, serde_json::json!({ "type": "load", "videoId": "x", "startMs": 5 }));
        let v = serde_json::to_value(YoutubeCommand::Test { video_id: TEST_VIDEO_ID.into() }).unwrap();
        assert_eq!(v["type"], "test");
    }

    #[tokio::test]
    async fn test_hub_resolves_ok_error_and_timeout() {
        let hub = YoutubeTestHub::default();
        let rx = hub.begin();
        hub.report(Ok(()));
        assert!(YoutubeTestHub::wait(rx, Duration::from_secs(1)).await.is_ok());

        let rx = hub.begin();
        hub.report(Err(153));
        let err = YoutubeTestHub::wait(rx, Duration::from_secs(1)).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::YoutubeEmbedBlocked { code: 153 });

        let rx = hub.begin();
        let err = YoutubeTestHub::wait(rx, Duration::from_millis(20)).await.unwrap_err();
        assert_eq!(err.code, ErrorCode::YoutubeEmbedBlocked { code: 0 });

        // A newer test supersedes an older one.
        let old = hub.begin();
        let _new = hub.begin();
        assert!(YoutubeTestHub::wait(old, Duration::from_secs(1)).await.is_err());
    }
}
