import { ExternalLink, ListMusic, Play, Shuffle } from "lucide-react";
import { useCallback, useMemo } from "react";
import { useParams } from "react-router";
import { LoadError } from "@/components/library/States";
import { EmptyState, Page } from "@/components/shell/Page";
import { Artwork, Mosaic } from "@/components/track/Artwork";
import { SourceMark } from "@/components/track/SourceMark";
import { TrackTable } from "@/components/track/TrackTable";
import { Button, Icon, Skeleton } from "@/components/ui";
import { format, strings } from "@/i18n";
import type { PlaylistDetail_Serialize as PlaylistDetail, Track_Serialize as Track } from "@/lib/bindings";
import { commands } from "@/lib/bindings";
import { isAppError } from "@/lib/errors";
import { formatCount, formatTotalDuration } from "@/lib/format";
import { usePlaylist, usePlaylistTracks } from "@/lib/queries";
import { toast } from "@/stores/toast";

const t = strings.pages.playlist;
const PREFETCH_ROWS = 60;
const playSoon = () => toast({ title: strings.library.playSoon });

function Header({ detail, artwork }: { detail: PlaylistDetail; artwork: Array<string | null> }) {
  const { playlist } = detail;
  const provider = playlist.provider;
  const meta = [
    playlist.owner || strings.library.unisonPlaylist,
    formatCount(playlist.trackCount),
    detail.durationMs ? formatTotalDuration(detail.durationMs) : null,
  ].filter(Boolean);
  const canPlay = detail.itemsAvailable && playlist.trackCount > 0;

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
              <Button variant="primary" onClick={playSoon}>
                <Icon as={Play} fill="currentColor" /> {strings.library.play}
              </Button>
              <Button variant="secondary" onClick={playSoon}>
                <Icon as={Shuffle} /> {strings.library.shuffle}
              </Button>
            </>
          ) : null}
          {detail.url && provider ? (
            <Button
              variant={canPlay ? "ghost" : "primary"}
              onClick={() => void commands.playlistOpenExternal(playlist.id)}
            >
              {format(t.openIn, { provider: strings.source[provider] })} <Icon as={ExternalLink} size={12} />
            </Button>
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
  const itemsAvailable = detail.data?.itemsAvailable ?? false;
  const tracks = usePlaylistTracks(id, itemsAvailable);

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
  } else if (!d.itemsAvailable) {
    body = (
      <div className="flex flex-1">
        <EmptyState icon={ListMusic} body={t.followedReason} />
      </div>
    );
  } else if (tracks.isError) {
    body = <LoadError error={tracks.error} onRetry={() => void tracks.refetch()} />;
  } else if (!data) {
    body = (
      <TrackTable
        rows={Array.from({ length: Math.min(Math.max(d.playlist.trackCount, 6), 12) }, () => undefined)}
        onPlay={playSoon}
        label={d.playlist.name}
      />
    );
  } else if (total === 0) {
    body = (
      <EmptyState
        icon={ListMusic}
        title={t.emptyPlaylistTitle}
        body={format(t.emptyPlaylistBody, {
          provider: d.playlist.provider ? strings.source[d.playlist.provider] : strings.library.unisonPlaylist,
        })}
      />
    );
  } else {
    body = <TrackTable rows={rows} onVisibleEnd={onVisibleEnd} onPlay={playSoon} label={d.playlist.name} />;
  }

  return (
    <div className="flex min-h-full flex-col px-8 pb-8">
      {d ? <Header detail={d} artwork={loaded.slice(0, 12).map((x) => x.artwork)} /> : <HeaderSkeleton />}
      {body}
    </div>
  );
}
