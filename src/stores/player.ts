import { create } from "zustand";

/** Player state mirror. Real state arrives from Rust via `player://state` (T-40 / T-42). */
type PlayerState = {
  hasTrack: boolean;
  playing: boolean;
  /** Toggles play/pause; no-op until something is loaded. */
  togglePlay: () => void;
};

export const usePlayerStore = create<PlayerState>((set, get) => ({
  hasTrack: false,
  playing: false,
  togglePlay: () => {
    if (!get().hasTrack) return;
    set((s) => ({ playing: !s.playing }));
  },
}));
