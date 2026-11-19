import { useDeferredValue, useState } from "react";
import { strings } from "@/i18n";
import type { LibrarySort, ProviderId } from "@/lib/bindings";

export type SourceFilter = "all" | ProviderId;

export type LibraryControls = {
  source: SourceFilter;
  setSource: (s: SourceFilter) => void;
  /** Raw input value. */
  filter: string;
  setFilter: (f: string) => void;
  /** Deferred value used for queries, so typing stays responsive. */
  query: string;
  sort: LibrarySort;
  setSort: (s: LibrarySort) => void;
  /** Provider for commands (`null` = all). */
  provider: ProviderId | null;
};

export function useLibraryControls(defaultSort: LibrarySort = "dateAdded"): LibraryControls {
  const [source, setSource] = useState<SourceFilter>("all");
  const [filter, setFilter] = useState("");
  const [sort, setSort] = useState<LibrarySort>(defaultSort);
  const query = useDeferredValue(filter.trim());
  return {
    source,
    setSource,
    filter,
    setFilter,
    query,
    sort,
    setSort,
    provider: source === "all" ? null : source,
  };
}

export type SortOption = { value: LibrarySort; label: string };

export const TRACK_SORTS: SortOption[] = [
  { value: "dateAdded", label: strings.library.sortDateAdded },
  { value: "title", label: strings.library.sortTitle },
  { value: "artist", label: strings.library.sortArtist },
];
