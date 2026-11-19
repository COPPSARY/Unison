import { create } from "zustand";

export type Theme = "dark" | "light";

type UiState = {
  theme: Theme;
  setTheme: (theme: Theme) => void;
};

function applyTheme(theme: Theme) {
  document.documentElement.dataset["theme"] = theme;
}

/** Dark is the default. Persisting the choice arrives with settings (T-06 / T-71). */
export const useUiStore = create<UiState>((set) => ({
  theme: "dark",
  setTheme: (theme) => {
    applyTheme(theme);
    set({ theme });
  },
}));

applyTheme(useUiStore.getState().theme);
