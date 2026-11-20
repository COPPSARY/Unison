import {
  QueryClient,
  keepPreviousData,
  useInfiniteQuery,
  useQuery,
} from "@tanstack/react-query";
import { commands, type LibrarySort, type SourceKind } from "@/lib/bindings";
import { isTauri } from "@/lib/window";

/** One client for the app. Commands read the local DB, so data stays fresh until invalidated. */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    defaultOptions: {
      queries: {
        staleTime: Infinity,
        refetchOnWindowFocus: false,
        retry: (count, error) =>
          count < 2 &&
          typeof error === "object" &&
          error !== null &&
          (error as { retryable?: unknown }).retryable === true,
      },
      mutations: { retry: false },
    },
  });
}

export const queryKeys = {
  appInfo: ["appInfo"] as const,
  extensions: ["extensions", "list"] as const,
  registries: ["extensions", "registries"] as const,
  registry: (url: string) => ["extensions", "registry", url] as const,
};

/** Queries are disabled outside Tauri (plain browser preview). */
const enabled = () => isTauri();

/** App + schema version (Settings → About). */
export function useAppInfo() {
  return useQuery({ queryKey: queryKeys.appInfo, queryFn: () => commands.appInfo(), enabled: enabled() });
}

// ---------------------------------------------------------------- Library views

/** Rows per liked-songs request; the table asks for more as you scroll. */
export const LIKED_PAGE = 300;

export type LibraryArgs = { provider: SourceKind | null; sort: LibrarySort; query: string };

const filterArg = (q: string) => (q ? q : null);

export function useLikedTracks({ provider, sort, query }: LibraryArgs) {
  return useInfiniteQuery({
    queryKey: ["liked", provider, sort, query],
    queryFn: ({ pageParam }) =>
      commands.libraryLiked(provider, sort, filterArg(query), pageParam, LIKED_PAGE),
    initialPageParam: 0,
    getNextPageParam: (last) => last.nextOffset ?? undefined,
    placeholderData: keepPreviousData,
    enabled: enabled(),
  });
}

export function useLikedStats(provider: SourceKind | null, query: string) {
  return useQuery({
    queryKey: ["likedStats", provider, query],
    queryFn: () => commands.libraryLikedStats(provider, filterArg(query)),
    placeholderData: keepPreviousData,
    enabled: enabled(),
  });
}

/** Library → Local files. Keys start with "local" so import events can refresh them. */
export function useLocalTracks({ sort, query }: { sort: LibrarySort; query: string }) {
  return useInfiniteQuery({
    queryKey: ["local", "tracks", sort, query],
    queryFn: ({ pageParam }) => commands.libraryLocal(sort, filterArg(query), pageParam, LIKED_PAGE),
    initialPageParam: 0,
    getNextPageParam: (last) => last.nextOffset ?? undefined,
    placeholderData: keepPreviousData,
    enabled: enabled(),
  });
}

export function useLocalStats(query: string) {
  return useQuery({
    queryKey: ["local", "stats", query],
    queryFn: () => commands.libraryLocalStats(filterArg(query)),
    placeholderData: keepPreviousData,
    enabled: enabled(),
  });
}

/** Settings → Local files: watched folders (refreshed with the rest of `["local"]`). */
export function useWatchedFolders() {
  return useQuery({
    queryKey: ["local", "folders"],
    queryFn: () => commands.localFoldersList(),
    enabled: enabled(),
  });
}

export function useAlbums({ provider, sort, query }: LibraryArgs) {
  return useQuery({
    queryKey: ["albums", provider, sort, query],
    queryFn: () => commands.libraryAlbums(provider, sort, filterArg(query)),
    placeholderData: keepPreviousData,
    enabled: enabled(),
  });
}

export function useArtists({ provider, sort, query }: LibraryArgs) {
  return useQuery({
    queryKey: ["artists", provider, sort, query],
    queryFn: () => commands.libraryArtists(provider, sort, filterArg(query)),
    placeholderData: keepPreviousData,
    enabled: enabled(),
  });
}

export function usePlaylists({ provider, sort, query }: LibraryArgs) {
  return useQuery({
    queryKey: ["playlists", provider, sort, query],
    queryFn: () => commands.playlistsList(provider, sort, filterArg(query)),
    placeholderData: keepPreviousData,
    enabled: enabled(),
  });
}

// ---------------------------------------------------------------- Playlist detail

export function usePlaylist(id: string) {
  return useQuery({
    queryKey: ["playlist", id],
    queryFn: () => commands.playlistGet(id),
    enabled: enabled(),
  });
}

/** Items in playlist order. */
export function usePlaylistTracks(id: string) {
  return useInfiniteQuery({
    queryKey: ["playlistTracks", id],
    queryFn: ({ pageParam }) => commands.playlistTracks(id, pageParam, LIKED_PAGE),
    initialPageParam: 0,
    getNextPageParam: (last) => last.nextOffset ?? undefined,
    enabled: enabled(),
  });
}

// ---------------------------------------------------------------- Player

// ---------------------------------------------------------------- Extensions

/** Installed extensions. */
export function useExtensions() {
  return useQuery({
    queryKey: queryKeys.extensions,
    queryFn: () => commands.extensionList(),
    enabled: enabled(),
  });
}

/** Registry links the user added. */
export function useRegistries() {
  return useQuery({
    queryKey: queryKeys.registries,
    queryFn: () => commands.extensionRegistries(),
    enabled: enabled(),
  });
}

/** What one saved registry lists (each entry previewed; nothing installed). */
export function useRegistry(url: string) {
  return useQuery({
    queryKey: queryKeys.registry(url),
    queryFn: () => commands.extensionRegistry(url),
    enabled: enabled(),
    staleTime: 10 * 60_000,
  });
}

/** One merged song list from every enabled extension; `query` should already be debounced. */
export function useSearchAll(query: string) {
  const q = query.trim();
  return useQuery({
    queryKey: ["extensions", "search", "all", q],
    queryFn: () => commands.extensionSearchAll(q),
    enabled: enabled() && q.length > 0,
    staleTime: 60_000,
    placeholderData: keepPreviousData,
  });
}

/** A song's track page (by any of its source keys). */
export function useSong(key: string) {
  return useQuery({ queryKey: ["song", key], queryFn: () => commands.extensionSong(key), enabled: enabled() });
}

/** Searches every enabled extension; `query` should already be debounced. */
export function useExtensionSearch(query: string) {
  const q = query.trim();
  return useQuery({
    queryKey: ["extensions", "search", q],
    queryFn: () => commands.extensionSearch(q),
    enabled: enabled() && q.length > 0,
    staleTime: 60_000,
    placeholderData: keepPreviousData,
  });
}
