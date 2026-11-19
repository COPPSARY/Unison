import { MicVocal } from "lucide-react";
import { CoverGrid } from "@/components/library/CoverGrid";
import { LibraryToolbar } from "@/components/library/LibraryToolbar";
import { useLibraryControls, type SortOption } from "@/components/library/controls";
import { Empty, GridSkeleton, LoadError, NoResults } from "@/components/library/States";
import { Page } from "@/components/shell/Page";
import { strings } from "@/i18n";
import { useArtists } from "@/lib/queries";

const t = strings.pages.artists;

const SORTS: SortOption[] = [
  { value: "dateAdded", label: strings.library.sortDateAdded },
  { value: "title", label: strings.library.sortName },
];

/** Followed artists and subscribed channels (spec §3 Artists index): circular tiles. */
export function Artists() {
  const controls = useLibraryControls();
  const artists = useArtists({ provider: controls.provider, sort: controls.sort, query: controls.query });
  const all = useArtists({ provider: null, sort: "dateAdded", query: "" });
  const filtered = controls.query !== "" || controls.source !== "all";

  if (all.data && all.data.length === 0) {
    return (
      <Page title={t.title}>
        <Empty icon={MicVocal} title={t.emptyTitle} body={t.emptyBody} />
      </Page>
    );
  }

  let body;
  if (artists.isError) body = <LoadError error={artists.error} onRetry={() => void artists.refetch()} />;
  else if (!artists.data) body = <GridSkeleton round />;
  else if (artists.data.length === 0 && filtered)
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
        items={artists.data.map((a) => ({
          id: a.id,
          href: `/artist/${a.id}`,
          title: a.name,
          meta: strings.source[a.provider],
          artwork: a.artwork,
          provider: null,
          kind: "artist",
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
