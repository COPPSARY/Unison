import {
  QueryClient,
  keepPreviousData,
  useInfiniteQuery,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { commands, type LibrarySort, type ProviderId } from "@/lib/bindings";
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
  accounts: ["accounts"] as const,
  spotifyHasClientId: ["spotify", "hasClientId"] as const,
  spotifyRedirectUris: ["spotify", "redirectUris"] as const,
};

/** Queries are disabled outside Tauri (plain browser preview). */
const enabled = () => isTauri();

/** App + schema version (Settings → About). */
export function useAppInfo() {
  return useQuery({ queryKey: queryKeys.appInfo, queryFn: () => commands.appInfo(), enabled: enabled() });
}

export function useAccounts() {
  return useQuery({
    queryKey: queryKeys.accounts,
    queryFn: () => commands.accountsList(),
    enabled: enabled(),
  });
}

export function useSpotifyHasClientId() {
  return useQuery({
    queryKey: queryKeys.spotifyHasClientId,
    queryFn: () => commands.spotifyHasClientId(),
    enabled: enabled(),
  });
}

export function useSpotifyRedirectUris() {
  return useQuery({
    queryKey: queryKeys.spotifyRedirectUris,
    queryFn: () => commands.spotifyRedirectUris(),
    enabled: enabled(),
  });
}

/** Saves the Client ID (if given) and runs the browser login. */
export function useConnectSpotify() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (clientId: string | null) => {
      if (clientId) await commands.spotifySetClientId(clientId);
      return commands.accountConnect("spotify" satisfies ProviderId);
    },
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: queryKeys.accounts });
      void qc.invalidateQueries({ queryKey: queryKeys.spotifyHasClientId });
    },
  });
}

/** Starts a background sync (all connected providers when `provider` is null). */
export function useSyncNow() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (provider: ProviderId | null) => commands.syncNow(provider),
    onSuccess: () => void qc.invalidateQueries({ queryKey: queryKeys.accounts }),
  });
}

// ---------------------------------------------------------------- Library views

/** Rows per liked-songs request; the table asks for more as you scroll. */
export const LIKED_PAGE = 300;

export type LibraryArgs = { provider: ProviderId | null; sort: LibrarySort; query: string };

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

export function useLikedStats(provider: ProviderId | null, query: string) {
  return useQuery({
    queryKey: ["likedStats", provider, query],
    queryFn: () => commands.libraryLikedStats(provider, filterArg(query)),
    placeholderData: keepPreviousData,
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

/** Items in playlist order. The first page may load them from the platform (lazy, T-15). */
export function usePlaylistTracks(id: string, itemsAvailable: boolean) {
  const qc = useQueryClient();
  return useInfiniteQuery({
    queryKey: ["playlistTracks", id],
    queryFn: async ({ pageParam }) => {
      const page = await commands.playlistTracks(id, pageParam, LIKED_PAGE);
      // The first load may have fetched items and updated count/duration.
      if (pageParam === 0) void qc.invalidateQueries({ queryKey: ["playlist", id] });
      return page;
    },
    initialPageParam: 0,
    getNextPageParam: (last) => last.nextOffset ?? undefined,
    enabled: enabled() && itemsAvailable,
  });
}
