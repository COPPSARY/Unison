import { QueryClient, useQuery } from "@tanstack/react-query";
import { commands } from "@/lib/bindings";
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
    },
  });
}

export const queryKeys = {
  appInfo: ["appInfo"] as const,
};

/** App + schema version (Settings → About). Disabled outside Tauri (plain browser preview). */
export function useAppInfo() {
  return useQuery({
    queryKey: queryKeys.appInfo,
    queryFn: () => commands.appInfo(),
    enabled: isTauri(),
  });
}
