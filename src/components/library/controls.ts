import { useDeferredValue, useState } from "react";
import { strings } from "@/i18n";
import type { LibrarySort, SourceKind } from "@/lib/bindings";

export type LibraryControls = {
  /** Raw input value. */
  filter: string;
  setFilter: (f: string) => void;
  /** Deferred value used for queries, so typing stays responsive. */
  query: string;
  sort: LibrarySort;
  setSort: (s: LibrarySort) => void;
  /** Source for commands (`null` = all). */
  provider: SourceKind | null;
};

export function useLibraryControls(defaultSort: LibrarySort = "dateAdded"): LibraryControls {
  const [filter, setFilter] = useState("");
  const [sort, setSort] = useState<LibrarySort>(defaultSort);
  const query = useDeferredValue(filter.trim());
  return {
    filter,
    setFilter,
    query,
    sort,
    setSort,
    provider: null,
  };
}

export type SortOption = { value: LibrarySort; label: string };

export const TRACK_SORTS: SortOption[] = [
  { value: "dateAdded", label: strings.library.sortDateAdded },
  { value: "title", label: strings.library.sortTitle },
  { value: "artist", label: strings.library.sortArtist },
];
