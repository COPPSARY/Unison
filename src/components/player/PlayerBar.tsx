import {
  ListMusic,
  Maximize2,
  Music,
  Pause,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Speaker,
  Volume2,
  VolumeX,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { Artwork } from "@/components/track/Artwork";
import { SourceMark } from "@/components/track/SourceMark";
import { Icon, IconButton, Skeleton, Tooltip } from "@/components/ui";
import { format, strings } from "@/i18n";
import { commands, type PlayerAction, type RepeatMode } from "@/lib/bindings";
import { cn } from "@/lib/cn";
import { errorMessage } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import { isTauri } from "@/lib/window";
import { usePlayerStore } from "@/stores/player";
import { toast } from "@/stores/toast";

const p = strings.player;

const control = (action: PlayerAction) => {
  if (!isTauri()) return;
  commands.playerControl(action).catch((err: unknown) => toast({ title: errorMessage(err), tone: "danger" }));
};

const NEXT_REPEAT: Record<RepeatMode, RepeatMode> = { off: "all", all: "one", one: "off" };

/** Seek bar: shows Rust's position; while dragging shows the drag value; seeks on release. */
function SeekBar({ position, duration, enabled }: { position: number; duration: number | null; enabled: boolean }) {
  const [drag, setDrag] = useState<number | null>(null);
  const max = duration ?? 0;
  const value = drag ?? Math.min(position, max);
  const pct = max > 0 ? (value / max) * 100 : 0;
  return (
    <div className="flex w-full items-center gap-2">
      <span className="w-10 text-right text-caption text-text-tertiary tabular-nums">{formatDuration(value)}</span>
      <input
        type="range"
        aria-label={p.seek}
        min={0}
        max={max || 1}
        step={1000}
        value={value}
        disabled={!enabled || !max}
        onChange={(e) => setDrag(Number(e.target.value))}
        onPointerUp={() => {
          if (drag !== null) control({ type: "seek", positionMs: Math.round(drag) });
          setDrag(null);
        }}
        onKeyUp={() => {
          if (drag !== null) control({ type: "seek", positionMs: Math.round(drag) });
          setDrag(null);
        }}
        className="u-range h-1 flex-1"
        style={{ "--pct": `${pct}%` } as React.CSSProperties}
      />
      <span className="w-10 text-caption text-text-tertiary tabular-nums">{formatDuration(max || 0)}</span>
    </div>
  );
}

function VolumeControl({ volume, enabled }: { volume: number; enabled: boolean }) {
  const [value, setValue] = useState(volume);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  useEffect(() => setValue(volume), [volume]);
  const send = (v: number) => {
    setValue(v);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => control({ type: "volume", volume: v }), 150);
  };
  return (
    <div className="flex items-center gap-1">
      <IconButton
        label={value === 0 ? p.volume : p.mute}
        size="sm"
        disabled={!enabled}
        icon={<Icon as={value === 0 ? VolumeX : Volume2} />}
        onClick={() => send(value === 0 ? 60 : 0)}
      />
      <input
        type="range"
        aria-label={p.volume}
        min={0}
        max={100}
        value={value}
        disabled={!enabled}
        onChange={(e) => send(Number(e.target.value))}
        className="u-range h-1 w-24"
        style={{ "--pct": `${value}%` } as React.CSSProperties}
      />
    </div>
  );
}

/** Floating glass player bar (72 px). Shows what Rust is playing; controls go to Rust. */
export function PlayerBar() {
  const state = usePlayerStore((s) => s.state);
  const togglePlay = usePlayerStore((s) => s.togglePlay);
  const hasTrack = !!state?.trackId;
  const loaded = hasTrack && (state.status === "playing" || state.status === "paused");
  const playing = state?.status === "playing";
  // A load can take a while (the extension finds the stream first): show the song right away.
  const loading = hasTrack && state.status === "loading";
  const track = state?.track ?? null;
  const source = state?.source ?? null;
  const repeat = state?.repeat ?? "off";

  return (
    <footer
      aria-label={p.playerBar}
      className="glass absolute inset-x-2 bottom-2 z-30 grid h-[72px] grid-cols-[1fr_minmax(320px,2fr)_1fr] items-center gap-4 rounded-panel px-3"
    >
      <div className="flex min-w-0 items-center gap-3">
        {track ? (
          <Artwork src={track.artwork} className="size-12 rounded-[8px]" />
        ) : (
          <div className="flex size-12 shrink-0 items-center justify-center rounded-[8px] bg-fill-tertiary text-text-tertiary">
            <Icon as={Music} size={20} />
          </div>
        )}
        {track ? (
          <div className="min-w-0">
            <p className="truncate text-body text-text">{track.title}</p>
            <p className="flex items-center gap-1.5 truncate text-callout text-text-muted">
              {loading ? <span className="shrink-0">{p.loading}</span> : null}
              {source ? (
                <Tooltip content={format(p.playingFrom, { provider: strings.source[source.kind] })}>
                  <span className="inline-flex">
                    <SourceMark provider={source.kind} />
                  </span>
                </Tooltip>
              ) : null}
              <span className="truncate">{track.artists.join(", ")}</span>
            </p>
          </div>
        ) : (
          <p className="truncate text-body text-text-muted">{p.notPlaying}</p>
        )}
      </div>

      <div className="flex flex-col items-center gap-1">
        <div className="flex items-center gap-1">
          <IconButton
            label={p.shuffle}
            size="sm"
            active={state?.shuffle ?? false}
            disabled={!hasTrack}
            icon={<Icon as={Shuffle} />}
            onClick={() => control({ type: "shuffle", on: !(state?.shuffle ?? false) })}
          />
          <IconButton label={p.previous} disabled={!hasTrack} icon={<Icon as={SkipBack} size={20} />} onClick={() => control({ type: "prev" })} />
          <IconButton
            label={loading ? p.loading : playing ? p.pause : p.play}
            variant="primary"
            size="lg"
            disabled={!loaded}
            icon={<Icon as={playing || loading ? Pause : Play} size={20} fill="currentColor" />}
            onClick={togglePlay}
          />
          <IconButton label={p.next} disabled={!hasTrack} icon={<Icon as={SkipForward} size={20} />} onClick={() => control({ type: "next" })} />
          <IconButton
            label={repeat === "one" ? p.repeatOne : repeat === "all" ? p.repeatAll : p.repeatOff}
            size="sm"
            active={repeat !== "off"}
            disabled={!hasTrack}
            icon={<Icon as={repeat === "one" ? Repeat1 : Repeat} />}
            onClick={() => control({ type: "repeat", mode: NEXT_REPEAT[repeat] })}
          />
        </div>
        {loading ? (
          <div role="progressbar" aria-label={p.loading} aria-busy="true" className="flex w-full items-center gap-2">
            <span className="w-10" />
            <Skeleton className="h-1 flex-1 rounded-full" />
            <span className="w-10" />
          </div>
        ) : (
          <SeekBar position={state?.positionMs ?? 0} duration={state?.durationMs ?? null} enabled={loaded && (state?.canSeek ?? false)} />
        )}
      </div>

      <div className={cn("flex items-center justify-end gap-1")}>
        <IconButton label={p.queue} disabled icon={<Icon as={ListMusic} />} />
        <IconButton label={p.devices} disabled icon={<Icon as={Speaker} />} />
        <VolumeControl volume={state?.volume ?? 80} enabled={isTauri()} />
        <IconButton label={p.expand} disabled icon={<Icon as={Maximize2} />} />
      </div>
    </footer>
  );
}
