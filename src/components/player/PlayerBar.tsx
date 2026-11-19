import {
  ListMusic,
  Maximize2,
  Music,
  Play,
  Repeat,
  Shuffle,
  SkipBack,
  SkipForward,
  Speaker,
  Volume2,
} from "lucide-react";
import { Icon, IconButton } from "@/components/ui";
import { strings } from "@/i18n";
import { usePlayerStore } from "@/stores/player";

const p = strings.player;

/** Floating glass player bar (72 px). Placeholder until playback lands (T-40 / T-42). */
export function PlayerBar() {
  const hasTrack = usePlayerStore((s) => s.hasTrack);
  const disabled = !hasTrack;

  return (
    <footer
      aria-label={p.playerBar}
      className="glass absolute inset-x-2 bottom-2 z-30 grid h-[72px] grid-cols-[1fr_minmax(320px,2fr)_1fr] items-center gap-4 rounded-panel px-3"
    >
      <div className="flex min-w-0 items-center gap-3">
        <div className="flex size-12 shrink-0 items-center justify-center rounded-[8px] bg-fill-tertiary text-text-tertiary">
          <Icon as={Music} size={20} />
        </div>
        <p className="truncate text-body text-text-muted">{p.notPlaying}</p>
      </div>

      <div className="flex flex-col items-center gap-1">
        <div className="flex items-center gap-1">
          <IconButton label={p.shuffle} size="sm" disabled={disabled} icon={<Icon as={Shuffle} />} />
          <IconButton label={p.previous} disabled={disabled} icon={<Icon as={SkipBack} size={20} />} />
          <IconButton
            label={p.play}
            variant="primary"
            size="lg"
            disabled={disabled}
            icon={<Icon as={Play} size={20} fill="currentColor" />}
          />
          <IconButton label={p.next} disabled={disabled} icon={<Icon as={SkipForward} size={20} />} />
          <IconButton label={p.repeat} size="sm" disabled={disabled} icon={<Icon as={Repeat} />} />
        </div>
        <div className="flex w-full items-center gap-2">
          <span className="w-10 text-right text-caption text-text-tertiary tabular-nums">0:00</span>
          <div
            role="slider"
            aria-label={p.seek}
            aria-valuemin={0}
            aria-valuemax={0}
            aria-valuenow={0}
            aria-disabled="true"
            className="h-1 flex-1 rounded-full bg-fill-secondary"
          />
          <span className="w-10 text-caption text-text-tertiary tabular-nums">0:00</span>
        </div>
      </div>

      <div className="flex items-center justify-end gap-1">
        <IconButton label={p.queue} disabled={disabled} icon={<Icon as={ListMusic} />} />
        <IconButton label={p.devices} disabled={disabled} icon={<Icon as={Speaker} />} />
        <IconButton label={p.volume} disabled={disabled} icon={<Icon as={Volume2} />} />
        <IconButton label={p.expand} disabled={disabled} icon={<Icon as={Maximize2} />} />
      </div>
    </footer>
  );
}
