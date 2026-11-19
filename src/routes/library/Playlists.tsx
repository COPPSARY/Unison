import { ListMusic } from "lucide-react";
import { CoverGrid } from "@/components/library/CoverGrid";
import { LibraryToolbar } from "@/components/library/LibraryToolbar";
import { useLibraryControls, type SortOption } from "@/components/library/controls";
import { Empty, GridSkeleton, LoadError, NoResults } from "@/components/library/States";
import { Page } from "@/components/shell/Page";
import { format, strings } from "@/i18n";
import { formatNumber } from "@/lib/format";
import { usePlaylists } from "@/lib/queries";

const t = strings.pages.playlists;

const SORTS: SortOption[] = [
  { value: "dateAdded", label: strings.library.sortDateAdded },
  { value: "title", label: strings.library.sortName },
  { value: "artist", label: strings.library.sortOwner },
];

/** Playlists from every source (spec §3 Playlists index). */
export function Playlists() {
  const controls = useLibraryControls();
  const playlists = usePlaylists({ provider: controls.provider, sort: controls.sort, query: controls.query });
  const all = usePlaylists({ provider: null, sort: "dateAdded", query: "" });
  const filtered = controls.query !== "" || controls.source !== "all";

  if (all.data && all.data.length === 0) {
    return (
      <Page title={t.title}>
        <Empty icon={ListMusic} title={t.emptyTitle} body={t.emptyBody} />
      </Page>
    );
  }

  let body;
  if (playlists.isError) body = <LoadError error={playlists.error} onRetry={() => void playlists.refetch()} />;
  else if (!playlists.data) body = <GridSkeleton />;
  else if (playlists.data.length === 0 && filtered)
    body = (
      <NoResults
        grid
        query={controls.query}
        onClear={() => {
          controls.setFilter("");
          controls.setSource("all");
        }}
      />
    );
  else
    body = (
      <CoverGrid
        items={playlists.data.map((p) => ({
          id: p.id,
          href: `/playlist/${p.id}`,
          title: p.name,
          meta: `${p.owner || strings.library.unisonPlaylist} · ${format(strings.library.tracksInPlaylist, {
            count: formatNumber(p.trackCount),
          })}`,
          artwork: p.artwork,
          provider: p.provider,
          kind: "playlist",
        }))}
      />
    );

  return (
    <Page title={t.title}>
      <LibraryToolbar controls={controls} sorts={SORTS} />
      {body}
    </Page>
  );
}
