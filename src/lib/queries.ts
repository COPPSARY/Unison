import { QueryClient, useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, type ProviderId } from "@/lib/bindings";
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
