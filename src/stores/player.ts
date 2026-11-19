import { create } from "zustand";
import { commands, type PlayerStateEvent_Serialize as PlayerState } from "@/lib/bindings";
import { isTauri } from "@/lib/window";

/** Mirror of Rust's player state (`player://state`). Rust decides what plays. */
type PlayerStore = {
  state: PlayerState | null;
  set: (state: PlayerState) => void;
  /** Space / play button: toggles when something is loaded (optimistic, Rust confirms). */
  togglePlay: () => void;
};

export const usePlayerStore = create<PlayerStore>((set, get) => ({
  state: null,
  set: (state) => set({ state }),
  togglePlay: () => {
    const s = get().state;
    if (!s?.trackId || (s.status !== "playing" && s.status !== "paused")) return;
    set({ state: { ...s, status: s.status === "playing" ? "paused" : "playing" } });
    if (isTauri()) void commands.playerControl({ type: "toggle" }).catch(() => undefined);
  },
}));

/** Selectors. */
export const selectHasTrack = (st: PlayerStore) => !!st.state?.trackId;
export const selectPlaying = (st: PlayerStore) => st.state?.status === "playing";
