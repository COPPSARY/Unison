import { create } from "zustand";

/** One running download (EXT-07), by job id. */
export type DownloadJob = { sourceKey: string; received: number; total: number | null };

type DownloadsState = {
  jobs: Record<number, DownloadJob>;
  started: (jobId: number, sourceKey: string) => void;
  progress: (jobId: number, received: number, total: number | null) => void;
  finished: (jobId: number) => void;
};

export const useDownloadsStore = create<DownloadsState>((set) => ({
  jobs: {},
  started: (jobId, sourceKey) =>
    set((s) => ({ jobs: { ...s.jobs, [jobId]: { sourceKey, received: 0, total: null } } })),
  progress: (jobId, received, total) =>
    set((s) => {
      const job = s.jobs[jobId];
      return job ? { jobs: { ...s.jobs, [jobId]: { ...job, received, total } } } : s;
    }),
  finished: (jobId) =>
    set((s) => ({ jobs: Object.fromEntries(Object.entries(s.jobs).filter(([id]) => Number(id) !== jobId)) })),
}));

/** The running download of a source, if any. */
export function useSourceDownload(sourceKey: string): DownloadJob | null {
  return useDownloadsStore((s) => Object.values(s.jobs).find((j) => j.sourceKey === sourceKey) ?? null);
}
