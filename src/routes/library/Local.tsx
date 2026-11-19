import { FilePlus, FolderPlus, HardDrive, Play, Shuffle } from "lucide-react";
import { useCallback, useMemo } from "react";
import { LibraryToolbar } from "@/components/library/LibraryToolbar";
import { useLibraryControls } from "@/components/library/controls";
import { LoadError, NoResults } from "@/components/library/States";
import { EmptyState, Page } from "@/components/shell/Page";
import { Mosaic } from "@/components/track/Artwork";
import { TrackTable } from "@/components/track/TrackTable";
import { Button, Icon, Skeleton } from "@/components/ui";
import { format, strings } from "@/i18n";
import type { Track_Serialize as Track } from "@/lib/bindings";
import { formatCount, formatTotalDuration } from "@/lib/format";
import { addFiles, addFolder, locateFile } from "@/lib/local";
import { playTracks } from "@/lib/playback";
import { LIKED_PAGE, useLocalStats, useLocalTracks } from "@/lib/queries";
import { useLocalImportStore, type ImportJob } from "@/stores/localImport";

const t = strings.pages.local;
const PREFETCH_ROWS = 60;
const intl = new Intl.NumberFormat();

function AddButtons({ size = "md" }: { size?: "sm" | "md" }) {
  return (
    <>
      <Button variant="secondary" size={size} onClick={addFiles}>
        <Icon as={FilePlus} /> {t.addFiles}
      </Button>
      <Button variant="secondary" size={size} onClick={addFolder}>
        <Icon as={FolderPlus} /> {t.addFolder}
      </Button>
    </>
  );
}

function jobLine(job: ImportJob) {
  if (job.total === null) return t.scanning;
  return format(t.importing, { done: intl.format(job.scanned), total: intl.format(job.total) });
}

function Header({
  artwork,
  stats,
  job,
  play,
}: {
  artwork: Array<string | null>;
  stats: { count: number; durationMs: number } | undefined;
  job: ImportJob | null;
  play: (shuffle: boolean) => void;
}) {
  return (
    <div className="flex items-end gap-6 pt-2 pb-6">
      <Mosaic srcs={artwork} className="size-36 shadow-glass" />
      <div className="flex min-w-0 flex-1 flex-col gap-3">
        <h1 className="text-large-title text-text">{t.title}</h1>
        <p className="text-callout text-text-muted tabular-nums" aria-live="polite">
          {stats ? (
            <>
              {formatCount(stats.count)} · {formatTotalDuration(stats.durationMs)}
              {job ? <> · {jobLine(job)}</> : null}
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
          <AddButtons />
        </div>
      </div>
    </div>
  );
}

/** Library → Local files (UP-11/12): imported files, Add files / Add folder, drop anywhere, Locate file…. */
export function Local() {
  const controls = useLibraryControls();
  const list = useLocalTracks({ sort: controls.sort, query: controls.query });
  const stats = useLocalStats(controls.query);
  const allStats = useLocalStats("");
  const job = useLocalImportStore((s) => s.job);

  const { data, hasNextPage, isFetchingNextPage, fetchNextPage } = list;
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

  if (allStats.data && allStats.data.count === 0) {
    return (
      <Page title={t.title}>
        <EmptyState
          icon={HardDrive}
          title={job ? jobLine(job) : t.emptyTitle}
          body={t.emptyBody}
          action={
            <div className="flex gap-2">
              <AddButtons size="sm" />
            </div>
          }
        />
      </Page>
    );
  }

  let body;
  if (list.isError) {
    body = <LoadError error={list.error} onRetry={() => void list.refetch()} />;
  } else if (!data) {
    body = (
      <TrackTable rows={Array.from({ length: Math.min(LIKED_PAGE, 12) }, () => undefined)} onPlay={() => undefined} label={t.title} />
    );
  } else if (total === 0 && controls.query !== "") {
    body = <NoResults query={controls.query} onClear={() => controls.setFilter("")} />;
  } else {
    body = (
      <TrackTable
        rows={rows}
        onVisibleEnd={onVisibleEnd}
        onPlay={(i) => void playTracks(loaded, i)}
        onLocate={(i) => {
          const track = loaded[i];
          if (track) void locateFile(track);
        }}
        label={t.title}
      />
    );
  }

  return (
    <div className="flex min-h-full flex-col px-8 pb-8">
      <Header
        artwork={loaded.slice(0, 12).map((x) => x.artwork)}
        stats={stats.data ? { count: stats.data.count, durationMs: stats.data.durationMs ?? 0 } : undefined}
        job={job}
        play={(shuffle) => void playTracks(loaded.filter((x) => x.available), 0, shuffle)}
      />
      <LibraryToolbar controls={controls} showSource={false} />
      {body}
    </div>
  );
}
