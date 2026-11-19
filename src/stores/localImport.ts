import { create } from "zustand";
import type { Issue } from "@/lib/bindings";

export type ImportJob = { jobId: number; scanned: number; total: number | null; imported: number };

type LocalImportState = {
  /** Files are being dragged over the window. */
  dragging: boolean;
  /** The import in progress, if any. */
  job: ImportJob | null;
  /** Per-file problems of the last finished import, shown in the details dialog. */
  issues: Issue[];
  issuesOpen: boolean;
  setDragging: (dragging: boolean) => void;
  progress: (job: ImportJob) => void;
  finish: (issues: Issue[]) => void;
  openIssues: () => void;
  closeIssues: () => void;
};

export const useLocalImportStore = create<LocalImportState>((set) => ({
  dragging: false,
  job: null,
  issues: [],
  issuesOpen: false,
  setDragging: (dragging) => set({ dragging }),
  progress: (job) => set({ job }),
  finish: (issues) => set({ job: null, issues }),
  openIssues: () => set({ issuesOpen: true }),
  closeIssues: () => set({ issuesOpen: false }),
}));
