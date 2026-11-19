import { Disc3 } from "lucide-react";
import { CoverGrid } from "@/components/library/CoverGrid";
import { LibraryToolbar } from "@/components/library/LibraryToolbar";
import { TRACK_SORTS, useLibraryControls } from "@/components/library/controls";
import { Empty, GridSkeleton, LoadError, NoResults } from "@/components/library/States";
import { Page } from "@/components/shell/Page";
import { strings } from "@/i18n";
import { useAlbums } from "@/lib/queries";

const t = strings.pages.albums;

/** Saved albums (spec §3 Albums index): 180 px cover grid. */
export function Albums() {
  const controls = useLibraryControls();
  const albums = useAlbums({ provider: controls.provider, sort: controls.sort, query: controls.query });
  const all = useAlbums({ provider: null, sort: "dateAdded", query: "" });
  const filtered = controls.query !== "" || controls.source !== "all";

  if (all.data && all.data.length === 0) {
    return (
      <Page title={t.title}>
        <Empty icon={Disc3} title={t.emptyTitle} body={t.emptyBody} />
      </Page>
    );
  }

  let body;
  if (albums.isError) body = <LoadError error={albums.error} onRetry={() => void albums.refetch()} />;
  else if (!albums.data) body = <GridSkeleton />;
  else if (albums.data.length === 0 && filtered)
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
        items={albums.data.map((a) => ({
          id: a.id,
          href: `/album/${a.id}`,
          title: a.name,
          meta: a.artists.join(", "),
          artwork: a.artwork,
          provider: a.provider,
          kind: "album",
        }))}
      />
    );

  return (
    <Page title={t.title}>
      <LibraryToolbar controls={controls} sorts={TRACK_SORTS} />
      {body}
    </Page>
  );
}
