import { useEffect, useRef, useState } from "react";
import { strings } from "@/i18n";
import { commands, events, type YoutubeCommand } from "@/lib/bindings";
import { isTauri } from "@/lib/window";
import { stateName, YT_STATE, type YTPlayer } from "@/lib/youtube";
import { useUiStore } from "@/stores/ui";
import { YouTubeHost } from "./YouTubeHost";

/** While playing, the position is reported this often so Rust can show progress. */
const POSITION_EVERY_MS = 1000;

type Current = { videoId: string; startMs: number; nonce: number };

const ms = (seconds: number) => Math.max(0, Math.round(seconds * 1000));

/**
 * The YouTubeEngine's webview half (UP-07). Always mounted in the shell; shows the official
 * IFrame player in a floating glass card (>= 200×200) while a video is loaded, executes
 * `player://youtube-command`, and reports state + position back to Rust.
 */
export function YouTubeDock() {
  const relay = useUiStore((st) => st.settings?.player.youtubeRelayUrl ?? null);
  const [current, setCurrent] = useState<Current | null>(null);
  const playerRef = useRef<YTPlayer | null>(null);
  const playingRef = useRef(false);

  const report = (state: number, error: number | null = null) => {
    const p = playerRef.current;
    const position = p ? ms(p.getCurrentTime()) : 0;
    const duration = p ? ms(p.getDuration()) : 0;
    void commands.youtubePlayerReport(stateName(state), position, duration > 0 ? duration : null, error);
  };

  // Register as the visible host while mounted.
  useEffect(() => {
    if (!isTauri()) return;
    void commands.youtubeHostReady(true);
    return () => void commands.youtubeHostReady(false);
  }, []);

  // Execute commands from Rust.
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const run = (cmd: YoutubeCommand) => {
      const p = playerRef.current;
      switch (cmd.type) {
        case "load":
          playerRef.current = null;
          setCurrent((c) => ({ videoId: cmd.videoId, startMs: cmd.startMs, nonce: (c?.nonce ?? 0) + 1 }));
          break;
        case "play":
          p?.playVideo();
          break;
        case "pause":
          p?.pauseVideo();
          break;
        case "seek":
          p?.seekTo(cmd.positionMs / 1000, true);
          break;
        case "volume":
          p?.setVolume(cmd.volume);
          break;
        case "stop":
          p?.stopVideo();
          playerRef.current = null;
          setCurrent(null);
          break;
        case "test":
          break; // handled by Settings → YouTube player
      }
    };
    void events.playerYoutubeCommand
      .listen(({ payload }) => run(payload))
      .then((fn) => (disposed ? fn() : (unlisten = fn)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Position updates while playing.
  useEffect(() => {
    if (!current) return;
    const timer = setInterval(() => {
      if (playingRef.current) report(YT_STATE.playing);
    }, POSITION_EVERY_MS);
    return () => clearInterval(timer);
  }, [current]);

  if (!current) return null;

  return (
    <div className="glass absolute bottom-[96px] left-4 z-20 rounded-panel p-2">
      <YouTubeHost
        key={`${current.videoId}:${current.nonce}`}
        title={strings.player.youtubeVideo}
        videoId={current.videoId}
        startMs={current.startMs}
        relay={relay}
        className="w-[356px]"
        onPlayer={(p) => {
          playerRef.current = p;
        }}
        onStateChange={(state) => {
          playingRef.current = state === YT_STATE.playing;
          report(state);
        }}
        onError={(code) => {
          playingRef.current = false;
          report(YT_STATE.unstarted, code);
        }}
      />
    </div>
  );
}
