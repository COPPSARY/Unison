import { ListMusic, Play, Shuffle } from "lucide-react";
import { useCallback, useMemo } from "react";
import { useParams } from "react-router";
import { LoadError } from "@/components/library/States";
import { EmptyState, Page } from "@/components/shell/Page";
import { Artwork, Mosaic } from "@/components/track/Artwork";
import { SourceMark } from "@/components/track/SourceMark";
import { TrackTable } from "@/components/track/TrackTable";
import { Button, Icon, Skeleton } from "@/components/ui";
import { strings } from "@/i18n";
import type { PlaylistDetail_Serialize as PlaylistDetail, Track_Serialize as Track } from "@/lib/bindings";
import { isAppError } from "@/lib/errors";
import { formatCount, formatTotalDuration } from "@/lib/format";
import { usePlaylist, usePlaylistTracks } from "@/lib/queries";
import { playTracks } from "@/lib/playback";
import { sourceKind } from "@/lib/sources";

const t = strings.pages.playlist;
const PREFETCH_ROWS = 60;

function Header({
  detail,
  artwork,
  play,
}: {
  detail: PlaylistDetail;
  artwork: Array<string | null>;
  play: (shuffle: boolean) => void;
}) {
  const { playlist } = detail;
  const provider = sourceKind(playlist.provider);
  const meta = [
    playlist.owner || strings.library.unisonPlaylist,
    formatCount(playlist.trackCount),
    detail.durationMs ? formatTotalDuration(detail.durationMs) : null,
  ].filter(Boolean);
  const canPlay = playlist.trackCount > 0;

  return (
    <div className="flex items-end gap-6 pt-2 pb-6">
      {playlist.artwork ? (
        <Artwork src={playlist.artwork} kind="playlist" className="size-36 rounded-card shadow-glass" />
      ) : (
        <Mosaic srcs={artwork} className="size-36 shadow-glass" />
      )}
      <div className="flex min-w-0 flex-1 flex-col gap-3">
        <h1 className="truncate text-large-title text-text">{playlist.name}</h1>
        <p className="flex items-center gap-2 text-callout text-text-muted tabular-nums">
          {provider ? <SourceMark provider={provider} /> : null}
          {meta.join(" · ")}
        </p>
        <div className="flex items-center gap-2">
          {canPlay ? (
            <>
              <Button variant="primary" onClick={() => play(false)}>
                <Icon as={Play} fill="currentColor" /> {strings.library.play}
              </Button>
              <Button variant="secondary" onClick={() => play(true)}>
                <Icon as={Shuffle} /> {strings.library.shuffle}
              </Button>
            </>
          ) : null}
        </div>
      </div>
    </div>
  );
}

function HeaderSkeleton() {
  return (
    <div className="flex items-end gap-6 pt-2 pb-6">
      <h1 className="sr-only">{t.title}</h1>
      <Skeleton className="size-36" />
      <div className="flex flex-1 flex-col gap-3">
        <Skeleton shape="text" className="h-6 w-64" />
        <Skeleton shape="text" className="w-40" />
        <Skeleton shape="text" className="h-8 w-48" />
      </div>
    </div>
  );
}

/** Playlist detail (spec §3 Library lists; T-15). */
export function Playlist() {
  const { id = "" } = useParams();
  const detail = usePlaylist(id);
  const tracks = usePlaylistTracks(id);

  const { data, hasNextPage, isFetchingNextPage, fetchNextPage } = tracks;
  const total = data?.pages[0]?.total ?? 0;
  const loaded = useMemo(() => data?.pages.flatMap((p) => p.items) ?? [], [data]);
  const rows = useMemo<Array<Track | undefined>>(
    () => Array.from({ length: total }, (_, i) => loaded[i]),
    [loaded, total],
  );
  const onVisibleEnd = useCallback(
    (last: number) => {
      if (hasNextPage && !isFetchingNextPage && last >= loaded.length - PREFETCH_ROWS) void fetchNextPage();
    },
    [hasNextPage, isFetchingNextPage, fetchNextPage, loaded.length],
  );

  if (detail.isError) {
    if (isAppError(detail.error) && detail.error.code.type === "notFound") {
      return (
        <Page title={t.title}>
          <EmptyState icon={ListMusic} title={t.emptyTitle} body={t.emptyBody} />
        </Page>
      );
    }
    return (
      <Page title={t.title}>
        <LoadError error={detail.error} onRetry={() => void detail.refetch()} />
      </Page>
    );
  }

  const d = detail.data;
  let body;
  if (!d) {
    body = null;
  } else if (tracks.isError) {
    body = <LoadError error={tracks.error} onRetry={() => void tracks.refetch()} />;
  } else if (!data) {
    body = (
      <TrackTable
        rows={Array.from({ length: Math.min(Math.max(d.playlist.trackCount, 6), 12) }, () => undefined)}
        onPlay={() => undefined}
        label={d.playlist.name}
      />
    );
  } else if (total === 0) {
    body = (
      <EmptyState
        icon={ListMusic}
        title={t.emptyPlaylistTitle}
        body={t.emptyPlaylistBody}
      />
    );
  } else {
    body = (
      <TrackTable
        rows={rows}
        onVisibleEnd={onVisibleEnd}
        onPlay={(i) => void playTracks(loaded, i)}
        label={d.playlist.name}
      />
    );
  }

  return (
    <div className="flex min-h-full flex-col px-8 pb-8">
      {d ? (
        <Header
          detail={d}
          artwork={loaded.slice(0, 12).map((x) => x.artwork)}
          play={(shuffle) => void playTracks(loaded.filter((x) => x.available), 0, shuffle)}
        />
      ) : (
        <HeaderSkeleton />
      )}
      {body}
    </div>
  );
}
