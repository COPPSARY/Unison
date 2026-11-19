import { create } from "zustand";
import type { ProviderId, SyncStage } from "@/lib/bindings";

export type SyncState = { stage: SyncStage; done: number; total: number | null };

type SyncStore = {
  /** Running syncs by provider (from `sync://progress`). */
  running: Partial<Record<ProviderId, SyncState>>;
  progress: (provider: ProviderId, state: SyncState) => void;
  finish: (provider: ProviderId) => void;
};

export const useSyncStore = create<SyncStore>((set) => ({
  running: {},
  progress: (provider, state) => set((s) => ({ running: { ...s.running, [provider]: state } })),
  finish: (provider) =>
    set((s) => ({
      running: Object.fromEntries(
        Object.entries(s.running).filter(([p]) => p !== provider),
      ) as SyncStore["running"],
    })),
}));
