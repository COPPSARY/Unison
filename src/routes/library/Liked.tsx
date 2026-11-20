import { Heart, Play, Shuffle } from "lucide-react";
import { useCallback, useMemo } from "react";
import { LibraryToolbar } from "@/components/library/LibraryToolbar";
import { useLibraryControls } from "@/components/library/controls";
import { Empty, LoadError, NoResults } from "@/components/library/States";
import { Page } from "@/components/shell/Page";
import { Mosaic } from "@/components/track/Artwork";
import { TrackTable } from "@/components/track/TrackTable";
import { Button, Icon, Skeleton } from "@/components/ui";
import { strings } from "@/i18n";
import type { Track_Serialize as Track } from "@/lib/bindings";
import { formatCount, formatTotalDuration } from "@/lib/format";
import { LIKED_PAGE, useLikedStats, useLikedTracks } from "@/lib/queries";
import { playTracks } from "@/lib/playback";

const t = strings.pages.liked;
/** Load the next page when the last visible row is this close to the loaded end. */
const PREFETCH_ROWS = 60;


function Header({
  artwork,
  stats,
  play,
}: {
  artwork: Array<string | null>;
  stats: { count: number; durationMs: number | null } | undefined;
  play: (shuffle: boolean) => void;
}) {
  return (
    <div className="flex items-end gap-6 pt-2 pb-6">
      <Mosaic srcs={artwork} className="size-36 shadow-glass" />
      <div className="flex min-w-0 flex-1 flex-col gap-3">
        <h1 className="text-large-title text-text">{t.title}</h1>
        <p className="text-callout text-text-muted tabular-nums">
          {stats ? (
            <>
              {formatCount(stats.count)} · {formatTotalDuration(stats.durationMs ?? 0)}
            </>
          ) : (
            <Skeleton shape="text" className="inline-block w-40" />
          )}
        </p>
        <div className="flex items-center gap-2">
          <Button variant="primary" onClick={() => play(false)} disabled={!stats?.count}>
            <Icon as={Play} fill="currentColor" /> {strings.library.play}
          </Button>
          <Button variant="secondary" onClick={() => play(true)} disabled={!stats?.count}>
            <Icon as={Shuffle} /> {strings.library.shuffle}
          </Button>
        </div>
      </div>
    </div>
  );
}

/** Liked songs (spec §3 Library lists): merged across sources, virtualised. */
export function Liked() {
  const controls = useLibraryControls();
  const args = { provider: controls.provider, sort: controls.sort, query: controls.query };
  const liked = useLikedTracks(args);
  const stats = useLikedStats(controls.provider, controls.query);
  const allStats = useLikedStats(null, "");

  const { data, hasNextPage, isFetchingNextPage, fetchNextPage } = liked;
  const total = data?.pages[0]?.total ?? 0;
  const loaded = useMemo(() => data?.pages.flatMap((p) => p.items) ?? [], [data]);
  const rows = useMemo<Array<Track | undefined>>(
    () => Array.from({ length: total }, (_, i) => loaded[i]),
    [loaded, total],
  );

  const onVisibleEnd = useCallback(
    (last: number) => {
      if (hasNextPage && !isFetchingNextPage && last >= loaded.length - PREFETCH_ROWS) {
        void fetchNextPage();
      }
    },
    [hasNextPage, isFetchingNextPage, fetchNextPage, loaded.length],
  );

  const filtered = controls.query !== "";
  const clear = () => {
    controls.setFilter("");
  };

  // Nothing liked anywhere: a single empty state, no toolbar.
  if (allStats.data && allStats.data.count === 0) {
    return (
      <Page title={t.title}>
        <Empty icon={Heart} title={t.emptyTitle} body={t.emptyBody} />
      </Page>
    );
  }

  let body;
  if (liked.isError) {
    body = <LoadError error={liked.error} onRetry={() => void liked.refetch()} />;
  } else if (!data) {
    body = (
      <TrackTable
        rows={Array.from({ length: Math.min(LIKED_PAGE, 12) }, () => undefined)}
        onPlay={() => undefined}
        label={t.title}
      />
    );
  } else if (total === 0 && filtered) {
    body = <NoResults query={controls.query} onClear={clear} />;
  } else {
    body = (
      <TrackTable rows={rows} onVisibleEnd={onVisibleEnd} onPlay={(i) => void playTracks(loaded, i)} label={t.title} />
    );
  }

  return (
    <div className="flex min-h-full flex-col px-8 pb-8">
      <Header
        artwork={loaded.slice(0, 12).map((x) => x.artwork)}
        stats={stats.data ? { count: stats.data.count, durationMs: stats.data.durationMs } : undefined}
        play={(shuffle) => void playTracks(loaded.filter((x) => x.available), 0, shuffle)}
      />
      <LibraryToolbar controls={controls} />
      {body}
    </div>
  );
}
