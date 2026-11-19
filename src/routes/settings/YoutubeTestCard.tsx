import { useEffect, useState } from "react";
import { YouTubeHost } from "@/components/player/YouTubeHost";
import { Button } from "@/components/ui";
import { strings } from "@/i18n";
import { commands, events } from "@/lib/bindings";
import { errorMessage } from "@/lib/errors";
import { isTauri } from "@/lib/window";
import { YT_STATE } from "@/lib/youtube";

const y = strings.settings.youtube;

type Result = { ok: true } | { ok: false; message: string } | null;

/**
 * Settings → YouTube player → Test playback (UP-03). Rust asks for a test through
 * `player://youtube-command`; this card shows the visible player and reports the outcome.
 */
export function YoutubeTestCard({ relay }: { relay?: string | null }) {
  const [videoId, setVideoId] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);
  const [result, setResult] = useState<Result>(null);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void events.playerYoutubeCommand
      .listen(({ payload }) => {
        if (payload.type === "test") setVideoId(payload.videoId);
      })
      .then((fn) => (disposed ? fn() : (unlisten = fn)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const run = () => {
    setTesting(true);
    setResult(null);
    setVideoId(null);
    commands
      .youtubeTestPlayback()
      .then(() => setResult({ ok: true }))
      .catch((err: unknown) => setResult({ ok: false, message: errorMessage(err) }))
      .finally(() => setTesting(false));
  };

  return (
    <div className="flex flex-col gap-3 px-4 py-3">
      <div className="flex items-center justify-between gap-4">
        <span className="text-body text-text">{y.title}</span>
        <div className="flex items-center gap-2">
          {videoId && !testing ? (
            <Button variant="ghost" size="sm" onClick={() => setVideoId(null)}>
              {y.close}
            </Button>
          ) : null}
          <Button variant="secondary" size="sm" onClick={run} disabled={testing}>
            {testing ? y.testing : y.test}
          </Button>
        </div>
      </div>
      {result ? (
        <p role={result.ok ? "status" : "alert"} className={result.ok ? "text-callout text-success" : "text-callout text-danger"}>
          {result.ok ? y.ok : result.message}
        </p>
      ) : null}
      {videoId ? (
        <YouTubeHost
          title={y.title}
          videoId={videoId}
          relay={relay ?? null}
          className="w-[356px] self-start"
          onStateChange={(state) => {
            if (state === YT_STATE.playing) void commands.youtubeTestReport(true, null);
          }}
          onError={(code) => void commands.youtubeTestReport(false, code)}
        />
      ) : null}
    </div>
  );
}
