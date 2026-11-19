import { useVirtualizer } from "@tanstack/react-virtual";
import { Clock3, Play } from "lucide-react";
import { useEffect, useRef, type KeyboardEvent } from "react";
import { useScrollContainer } from "@/components/shell/ScrollContainer";
import { Icon, Skeleton, Tooltip } from "@/components/ui";
import { format, strings } from "@/i18n";
import type { Track_Serialize as Track } from "@/lib/bindings";
import { cn } from "@/lib/cn";
import { formatDate, formatDuration } from "@/lib/format";
import { Artwork } from "./Artwork";
import { SourceMarks } from "./SourceMark";

const ROW_PX = 56;
const c = strings.library.col;

/** Column template; Album and Date added appear once the table is wide enough. */
const GRID =
  "grid grid-cols-[40px_minmax(0,1fr)_48px_56px] @[760px]:grid-cols-[40px_minmax(0,1.4fr)_minmax(0,1fr)_48px_112px_56px] items-center gap-4 px-3";

export type TrackTableProps = {
  /** Length = total rows; `undefined` entries are not loaded yet (rendered as skeletons). */
  rows: Array<Track | undefined>;
  /** Called with the last visible index so the caller can load more. */
  onVisibleEnd?: (lastIndex: number) => void;
  onPlay: (index: number) => void;
  label: string;
};

function Row({
  track,
  index,
  onPlay,
}: {
  track: Track;
  index: number;
  onPlay: (index: number) => void;
}) {
  const title = (
    <div className="flex min-w-0 items-center gap-3">
      <Artwork src={track.artwork} className={cn("size-10", !track.available && "opacity-40")} />
      <div className="min-w-0">
        <p className={cn("truncate text-body", track.available ? "text-text" : "text-text-disabled")}>
          {track.title}
        </p>
        <p className={cn("truncate text-callout", track.available ? "text-text-muted" : "text-text-disabled")}>
          {track.artists.join(", ")}
        </p>
      </div>
    </div>
  );

  return (
    <>
      <div className="relative flex h-full items-center justify-end">
        <span className="text-callout text-text-tertiary tabular-nums group-hover:invisible group-focus-visible:invisible">
          {index + 1}
        </span>
        {track.available ? (
          <button
            type="button"
            tabIndex={-1}
            aria-label={format(strings.library.playTrack, { title: track.title })}
            onClick={(e) => {
              // The row's own click plays too; don't start the same song twice.
              e.stopPropagation();
              onPlay(index);
            }}
            className="u-tone invisible absolute right-0 flex size-7 items-center justify-center rounded-full text-text group-hover:visible group-focus-visible:visible hover:bg-fill-tertiary"
          >
            <Icon as={Play} size={12} fill="currentColor" />
          </button>
        ) : null}
      </div>
      {track.available ? (
        title
      ) : (
        <Tooltip content={strings.library.unavailable} side="top">
          {title}
        </Tooltip>
      )}
      <p className="hidden truncate text-callout text-text-muted @[760px]:block">{track.album}</p>
      <SourceMarks providers={track.sources.map((s) => s.provider)} />
      <p className="hidden text-callout text-text-muted tabular-nums @[760px]:block">{formatDate(track.addedAt)}</p>
      <p className="text-right text-callout text-text-muted tabular-nums">{formatDuration(track.durationMs)}</p>
    </>
  );
}

function SkeletonRow() {
  return (
    <>
      <Skeleton shape="text" className="ml-auto w-5" />
      <div className="flex items-center gap-3">
        <Skeleton className="size-10 rounded-[6px]" />
        <div className="flex flex-1 flex-col gap-2">
          <Skeleton shape="text" className="w-2/5" />
          <Skeleton shape="text" className="w-1/4" />
        </div>
      </div>
      <Skeleton shape="text" className="hidden w-3/5 @[760px]:block" />
      <Skeleton shape="text" className="w-4" />
      <Skeleton shape="text" className="hidden w-16 @[760px]:block" />
      <Skeleton shape="text" className="ml-auto w-8" />
    </>
  );
}

/** Virtualised track list (spec §3 Library lists): handles 10,000+ rows. */
export function TrackTable({ rows, onVisibleEnd, onPlay, label }: TrackTableProps) {
  const scrollRef = useScrollContainer();
  const listRef = useRef<HTMLDivElement>(null);
  // Distance from the top of the scroll container to the first row (header + toolbar above).
  const margin = listRef.current?.offsetTop ?? 0;

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef?.current ?? null,
    estimateSize: () => ROW_PX,
    overscan: 12,
    scrollMargin: margin,
    initialRect: { width: 1000, height: 800 },
  });
  const items = virtualizer.getVirtualItems();
  const lastIndex = items.at(-1)?.index ?? 0;

  useEffect(() => {
    onVisibleEnd?.(lastIndex);
  }, [lastIndex, onVisibleEnd]);

  const focusRow = (index: number) => {
    if (index < 0 || index >= rows.length) return;
    virtualizer.scrollToIndex(index, { align: "auto" });
    requestAnimationFrame(() => {
      listRef.current?.querySelector<HTMLElement>(`[data-row="${index}"]`)?.focus();
    });
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>, index: number) => {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      focusRow(index + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      focusRow(index - 1);
    } else if (e.key === "Home") {
      e.preventDefault();
      focusRow(0);
    } else if (e.key === "End") {
      e.preventDefault();
      focusRow(rows.length - 1);
    } else if (e.key === "Enter" && rows[index]?.available) {
      e.preventDefault();
      onPlay(index);
    }
  };

  return (
    <div role="grid" aria-label={label} aria-rowcount={rows.length + 1} className="@container">
      <div
        role="row"
        className={cn(
          GRID,
          "sticky top-10 z-10 h-9 border-b border-border bg-background text-footnote font-medium text-text-tertiary",
        )}
      >
        <span role="columnheader" className="text-right">
          {c.number}
        </span>
        <span role="columnheader">{c.title}</span>
        <span role="columnheader" className="hidden @[760px]:block">
          {c.album}
        </span>
        <span role="columnheader">{c.source}</span>
        <span role="columnheader" className="hidden @[760px]:block">
          {c.dateAdded}
        </span>
        <span role="columnheader" className="flex justify-end" aria-label={c.duration}>
          <Icon as={Clock3} size={12} />
        </span>
      </div>

      <div ref={listRef} className="relative" style={{ height: virtualizer.getTotalSize() }}>
        {items.map((item) => {
          const track = rows[item.index];
          return (
            <div
              key={item.key}
              role="row"
              aria-rowindex={item.index + 2}
              data-row={item.index}
              tabIndex={track ? 0 : -1}
              onKeyDown={(e) => onKeyDown(e, item.index)}
              // A single click anywhere on the row plays it (owner's choice over §3's double-click).
              onClick={() => track?.available && onPlay(item.index)}
              className={cn(
                GRID,
                "group u-tone absolute inset-x-0 top-0 rounded-[8px] outline-none",
                track && "hover:bg-fill-quaternary focus-visible:bg-fill-tertiary",
                track?.available && "cursor-pointer",
              )}
              style={{ height: ROW_PX, transform: `translateY(${item.start - margin}px)` }}
            >
              {track ? <Row track={track} index={item.index} onPlay={onPlay} /> : <SkeletonRow />}
            </div>
          );
        })}
      </div>
    </div>
  );
}
