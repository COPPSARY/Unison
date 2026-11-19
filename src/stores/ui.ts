import { create } from "zustand";
import { applyAccent, type Rgb } from "@/lib/accent";

export type Theme = "dark" | "light";

type UiState = {
  theme: Theme;
  /** Colour of the current artwork; null = fallback violet. Set by playback (T-42). */
  accentSource: Rgb | null;
  /** Glass frost 0..1 (more blur + tint). User setting in T-71. */
  glassFrost: number;
  setTheme: (theme: Theme) => void;
  setAccentSource: (source: Rgb | null) => void;
  setGlassFrost: (frost: number) => void;
};

const root = () => document.documentElement;

/** Dark is the default. Persisting these choices arrives with settings (T-06 / T-71). */
export const useUiStore = create<UiState>((set, get) => ({
  theme: "dark",
  accentSource: null,
  glassFrost: 0.5,
  setTheme: (theme) => {
    root().dataset["theme"] = theme;
    applyAccent(get().accentSource, theme);
    set({ theme });
  },
  setAccentSource: (accentSource) => {
    applyAccent(accentSource, get().theme);
    set({ accentSource });
  },
  setGlassFrost: (frost) => {
    const glassFrost = Math.min(Math.max(frost, 0), 1);
    root().style.setProperty("--glass-frost", String(glassFrost));
    set({ glassFrost });
  },
}));

const initial = useUiStore.getState();
root().dataset["theme"] = initial.theme;
applyAccent(initial.accentSource, initial.theme);
