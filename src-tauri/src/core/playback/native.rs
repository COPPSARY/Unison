//! NativeEngine (addendum §1, UP-05): plays local files (and, from UP-06, HTTP streams) inside
//! Unison with symphonia decoding and rodio/cpal output. rodio's output handles are not `Send`,
//! so all audio lives on one dedicated thread; the engine sends it commands and awaits replies.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use async_trait::async_trait;
use rodio::cpal::traits::HostTrait;
use rodio::decoder::DecoderError;
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Source};
use tokio::sync::oneshot;

use super::engine::{Engine, EngineState, PlaySource, UnavailableReason};
use crate::error::{AppError, AppResult, ErrorCode};
use crate::model::SourceKind;

/// File types symphonia decodes in this build (rodio features mp3, flac, wav, vorbis, mp4/aac).
pub const PLAYABLE_EXTENSIONS: [&str; 7] = ["mp3", "flac", "wav", "ogg", "oga", "m4a", "aac"];

/// Checks a local file before loading it.
pub fn check_local_file(path: &Path) -> Result<(), UnavailableReason> {
    if !path.is_file() {
        return Err(UnavailableReason::FileMissing);
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if PLAYABLE_EXTENSIONS.contains(&ext.as_str()) {
        Ok(())
    } else {
        Err(UnavailableReason::UnsupportedFormat)
    }
}

/// Perceptual volume: UI 0–100 → amplitude (squared curve), times a ReplayGain factor.
pub fn amplitude(volume: u8, gain_db: Option<f32>) -> f32 {
    let base = (f32::from(volume.min(100)) / 100.0).powi(2);
    let gain = gain_db.map_or(1.0, |db| 10f32.powf(db / 20.0)).clamp(0.0, 2.0);
    base * gain
}

fn decoder_error(err: DecoderError) -> AppError {
    match err {
        DecoderError::UnrecognizedFormat => AppError::new(ErrorCode::UnsupportedFormat, "unrecognised audio format"),
        other => AppError::new(ErrorCode::UnsupportedFormat, format!("decode: {other}")),
    }
}

/// Opens a file and returns a seekable decoder plus its duration (no output device needed).
pub fn open_decoder(path: &Path) -> AppResult<(Decoder<std::io::BufReader<File>>, Option<Duration>)> {
    let file = File::open(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => AppError::new(ErrorCode::FileMissing, "file not found"),
        _ => AppError::internal(format!("open audio file: {e}")),
    })?;
    let decoder = Decoder::try_from(file).map_err(decoder_error)?;
    let duration = decoder.total_duration();
    Ok((decoder, duration))
}

/// Names of the output devices (for Settings → Playback).
pub fn output_device_names() -> Vec<String> {
    rodio::cpal::default_host()
        .output_devices()
        .map(|devices| devices.map(|d| d.to_string()).collect())
        .unwrap_or_default()
}

fn open_sink(device: Option<&str>) -> AppResult<MixerDeviceSink> {
    let unavailable = |why: String| AppError::new(ErrorCode::OutputDeviceUnavailable, why);
    let mut sink = match device {
        None => DeviceSinkBuilder::open_default_sink().map_err(|e| unavailable(format!("default output: {e}")))?,
        Some(name) => {
            let found = rodio::cpal::default_host()
                .output_devices()
                .map_err(|e| unavailable(format!("list outputs: {e}")))?
                .find(|d| d.to_string() == name)
                .ok_or_else(|| unavailable(format!("output device {name:?} not found")))?;
            DeviceSinkBuilder::from_device(found)
                .and_then(|b| b.open_stream())
                .map_err(|e| unavailable(format!("open output: {e}")))?
        }
    };
    sink.log_on_drop(false);
    Ok(sink)
}

type Reply<T> = oneshot::Sender<AppResult<T>>;

enum Cmd {
    Load { path: PathBuf, start_ms: u32, reply: Reply<()> },
    Play,
    Pause,
    Stop,
    Seek { ms: u32, reply: Reply<()> },
    Volume { volume: u8 },
    Gain { db: Option<f32> },
    SetOutput { device: Option<String>, reply: Reply<()> },
    State { reply: oneshot::Sender<EngineState> },
}

/// Everything owned by the audio thread.
struct AudioThread {
    sink: Option<MixerDeviceSink>,
    player: Option<rodio::Player>,
    device: Option<String>,
    path: Option<PathBuf>,
    duration: Option<Duration>,
    volume: u8,
    gain_db: Option<f32>,
}

impl AudioThread {
    fn ensure_sink(&mut self) -> AppResult<()> {
        if self.sink.is_none() {
            self.sink = Some(open_sink(self.device.as_deref())?);
        }
        Ok(())
    }

    fn apply_volume(&self) {
        if let Some(p) = &self.player {
            p.set_volume(amplitude(self.volume, self.gain_db) as rodio::Float);
        }
    }

    fn load(&mut self, path: PathBuf, start_ms: u32, paused: bool) -> AppResult<()> {
        let (decoder, duration) = open_decoder(&path)?;
        self.ensure_sink()?;
        if let Some(old) = self.player.take() {
            old.stop();
        }
        let sink = self.sink.as_ref().ok_or_else(|| AppError::internal("no audio output"))?;
        let player = rodio::Player::connect_new(sink.mixer());
        player.append(decoder);
        if start_ms > 0 {
            let _ = player.try_seek(Duration::from_millis(u64::from(start_ms)));
        }
        if paused {
            player.pause();
        } else {
            player.play();
        }
        self.player = Some(player);
        self.path = Some(path);
        self.duration = duration;
        self.apply_volume();
        Ok(())
    }

    fn state(&self) -> EngineState {
        match &self.player {
            None => EngineState::default(),
            Some(p) => {
                let pos = u32::try_from(p.get_pos().as_millis()).unwrap_or(u32::MAX);
                let ended = p.empty();
                EngineState {
                    position_ms: pos,
                    duration_ms: self.duration.and_then(|d| u32::try_from(d.as_millis()).ok()),
                    playing: !ended && !p.is_paused(),
                    ended,
                }
            }
        }
    }

    fn set_output(&mut self, device: Option<String>) -> AppResult<()> {
        // Remember where we are, reopen on the new device, resume at the same spot.
        let resume = self.player.as_ref().map(|p| (p.get_pos(), p.is_paused(), p.empty()));
        let new_sink = open_sink(device.as_deref())?;
        if let Some(p) = self.player.take() {
            p.stop();
        }
        self.sink = Some(new_sink);
        self.device = device;
        if let (Some(path), Some((pos, paused, ended))) = (self.path.clone(), resume) {
            if !ended {
                let ms = u32::try_from(pos.as_millis()).unwrap_or(0);
                self.load(path, ms, paused)?;
            }
        }
        Ok(())
    }

    fn run(mut self, rx: mpsc::Receiver<Cmd>) {
        while let Ok(cmd) = rx.recv() {
            match cmd {
                Cmd::Load { path, start_ms, reply } => {
                    let _ = reply.send(self.load(path, start_ms, false));
                }
                Cmd::Play => {
                    if let Some(p) = &self.player {
                        p.play();
                    }
                }
                Cmd::Pause => {
                    if let Some(p) = &self.player {
                        p.pause();
                    }
                }
                Cmd::Stop => {
                    if let Some(p) = self.player.take() {
                        p.stop();
                    }
                    self.path = None;
                    self.duration = None;
                }
                Cmd::Seek { ms, reply } => {
                    let result = match &self.player {
                        Some(p) => p
                            .try_seek(Duration::from_millis(u64::from(ms)))
                            .map_err(|e| AppError::internal(format!("seek: {e}"))),
                        None => Ok(()),
                    };
                    let _ = reply.send(result);
                }
                Cmd::Volume { volume } => {
                    self.volume = volume.min(100);
                    self.apply_volume();
                }
                Cmd::Gain { db } => {
                    self.gain_db = db;
                    self.apply_volume();
                }
                Cmd::SetOutput { device, reply } => {
                    let _ = reply.send(self.set_output(device));
                }
                Cmd::State { reply } => {
                    let _ = reply.send(self.state());
                }
            }
        }
    }
}

/// Handle to the audio thread. Cheap to share behind an `Arc`.
pub struct NativeEngine {
    tx: std::sync::Mutex<mpsc::Sender<Cmd>>,
}

impl NativeEngine {
    /// Spawns the audio thread. The output device opens lazily on first load.
    pub fn spawn(output_device: Option<String>, volume: u8) -> AppResult<Self> {
        let (tx, rx) = mpsc::channel();
        let thread = AudioThread {
            sink: None,
            player: None,
            device: output_device,
            path: None,
            duration: None,
            volume,
            gain_db: None,
        };
        std::thread::Builder::new()
            .name("unison-audio".into())
            .spawn(move || thread.run(rx))
            .map_err(|e| AppError::internal(format!("spawn audio thread: {e}")))?;
        Ok(Self { tx: std::sync::Mutex::new(tx) })
    }

    fn send(&self, cmd: Cmd) -> AppResult<()> {
        self.tx
            .lock()
            .map_err(|_| AppError::internal("audio thread lock poisoned"))?
            .send(cmd)
            .map_err(|_| AppError::internal("audio thread stopped"))
    }

    async fn ask<T>(&self, make: impl FnOnce(Reply<T>) -> Cmd) -> AppResult<T> {
        let (reply, rx) = oneshot::channel();
        self.send(make(reply))?;
        rx.await.map_err(|_| AppError::internal("audio thread did not answer"))?
    }

    /// Plays a file path directly (used by Test sound and, later, the local resolver).
    pub async fn load_path(&self, path: PathBuf, start_ms: u32) -> AppResult<()> {
        self.ask(|reply| Cmd::Load { path, start_ms, reply }).await
    }

    /// ReplayGain for the next/current item (dB; `None` = none).
    pub fn set_gain(&self, db: Option<f32>) -> AppResult<()> {
        self.send(Cmd::Gain { db })
    }

    /// Switches output device mid-track (`None` = system default).
    pub async fn set_output(&self, device: Option<String>) -> AppResult<()> {
        self.ask(|reply| Cmd::SetOutput { device, reply }).await
    }
}

#[async_trait]
impl Engine for NativeEngine {
    fn kind(&self) -> SourceKind {
        SourceKind::Local
    }

    async fn can_play(&self, source: &PlaySource) -> Result<(), UnavailableReason> {
        match source.kind {
            SourceKind::Local => check_local_file(Path::new(&source.url)),
            // HTTP streams (Audius) arrive with UP-06.
            _ => Err(UnavailableReason::NoSource),
        }
    }

    async fn load(&self, source: &PlaySource, start_ms: u32) -> AppResult<()> {
        self.load_path(PathBuf::from(&source.url), start_ms).await
    }

    async fn play(&self) -> AppResult<()> {
        self.send(Cmd::Play)
    }

    async fn pause(&self) -> AppResult<()> {
        self.send(Cmd::Pause)
    }

    async fn stop(&self) -> AppResult<()> {
        self.send(Cmd::Stop)
    }

    async fn seek(&self, position_ms: u32) -> AppResult<()> {
        self.ask(|reply| Cmd::Seek { ms: position_ms, reply }).await
    }

    async fn set_volume(&self, volume: u8) -> AppResult<()> {
        self.send(Cmd::Volume { volume })
    }

    async fn state(&self) -> AppResult<EngineState> {
        let (reply, rx) = oneshot::channel();
        self.send(Cmd::State { reply })?;
        rx.await.map_err(|_| AppError::internal("audio thread did not answer"))
    }
}

/// Writes a short quiet sine tone as WAV (Settings → Test sound, and tests).
pub fn write_test_tone(path: &Path, seconds: f32, amplitude: f32) -> AppResult<()> {
    let tone = rodio::source::SineWave::new(440.0)
        .take_duration(Duration::from_secs_f32(seconds))
        .amplify(amplitude as rodio::Float);
    rodio::wav_to_file(tone, path).map_err(|e| AppError::internal(format!("write test tone: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(dir: &Path, name: &str, secs: f32) -> PathBuf {
        let path = dir.join(name);
        write_test_tone(&path, secs, 0.0).unwrap();
        path
    }

    #[test]
    fn decodes_wav_with_duration_and_seeks() {
        let dir = tempfile::tempdir().unwrap();
        let path = tone(dir.path(), "tone.wav", 3.0);
        let (mut decoder, duration) = open_decoder(&path).unwrap();
        let ms = duration.unwrap().as_millis();
        assert!((2900..=3100).contains(&ms), "duration {ms} ms");
        decoder.try_seek(Duration::from_secs(2)).unwrap();
        // Roughly one second of samples left after seeking to 2 s.
        let rate = f64::from(decoder.sample_rate().get());
        let channels = f64::from(decoder.channels().get());
        let left = decoder.count() as f64 / rate / channels;
        assert!((0.8..=1.2).contains(&left), "left {left} s");
    }

    #[test]
    fn can_play_rules() {
        let dir = tempfile::tempdir().unwrap();
        let wav = tone(dir.path(), "ok.wav", 0.2);
        assert_eq!(check_local_file(&wav), Ok(()));
        assert_eq!(check_local_file(&dir.path().join("gone.mp3")), Err(UnavailableReason::FileMissing));
        let opus = dir.path().join("x.opus");
        std::fs::write(&opus, b"not audio").unwrap();
        assert_eq!(check_local_file(&opus), Err(UnavailableReason::UnsupportedFormat));
    }

    #[test]
    fn garbage_with_a_known_extension_is_unsupported_format() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake.mp3");
        std::fs::write(&fake, vec![0u8; 4096]).unwrap();
        let err = open_decoder(&fake).err().unwrap();
        assert_eq!(err.code, ErrorCode::UnsupportedFormat);
        let missing = open_decoder(&dir.path().join("none.flac")).err().unwrap();
        assert_eq!(missing.code, ErrorCode::FileMissing);
    }

    #[test]
    fn volume_curve_and_replaygain() {
        assert_eq!(amplitude(100, None), 1.0);
        assert_eq!(amplitude(0, None), 0.0);
        assert!((amplitude(50, None) - 0.25).abs() < 1e-6);
        assert!((amplitude(100, Some(-6.0)) - 0.501).abs() < 0.01);
        assert_eq!(amplitude(100, Some(40.0)), 2.0, "boost clamped");
    }

    /// Plays through the real default output (silent tone). Run on a PC with audio:
    /// `cargo test -- --ignored native_engine_plays_seeks_and_switches_output`
    #[tokio::test]
    #[ignore = "needs an audio output device"]
    async fn native_engine_plays_seeks_and_switches_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = tone(dir.path(), "tone.wav", 4.0);
        let engine = NativeEngine::spawn(None, 50).unwrap();
        let source = PlaySource { kind: SourceKind::Local, item_id: "x".into(), url: path.to_string_lossy().into() };
        assert_eq!(engine.can_play(&source).await, Ok(()));
        engine.load(&source, 0).await.unwrap();
        tokio::time::sleep(Duration::from_millis(400)).await;
        let s = engine.state().await.unwrap();
        assert!(s.playing && s.position_ms > 100, "{s:?}");
        assert!(s.duration_ms.unwrap() > 3_900);

        engine.seek(2_500).await.unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(engine.state().await.unwrap().position_ms >= 2_500);

        // Switch output mid-track (to the default device by name) and keep the position.
        let first = output_device_names().into_iter().next();
        engine.set_output(first).await.unwrap();
        let after = engine.state().await.unwrap();
        assert!(after.position_ms >= 2_400 && after.playing, "{after:?}");

        engine.pause().await.unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!engine.state().await.unwrap().playing);
        engine.play().await.unwrap();
        tokio::time::sleep(Duration::from_millis(1_800)).await;
        assert!(engine.state().await.unwrap().ended, "short file reaches the end");
    }
}
