import { create } from "zustand";
import { commands, type Settings_Serialize as Settings } from "@/lib/bindings";
import { applyAccent, type Rgb } from "@/lib/accent";
import { isTauri } from "@/lib/window";

export type Theme = "dark" | "light";

/** Sidebar sections that can be collapsed (keys stored in settings). */
export type SidebarSection = "listen" | "library" | "playlists";

type UiState = {
  theme: Theme;
  /** Colour of the current artwork; null = fallback violet. Set by playback (T-42). */
  accentSource: Rgb | null;
  /** Glass frost 0..1 (more blur + tint). */
  glassFrost: number;
  sidebarCollapsed: string[];
  /** Last settings loaded from / saved to Rust; other fields are carried through on save. */
  settings: Settings | null;
  setTheme: (theme: Theme) => void;
  setAccentSource: (source: Rgb | null) => void;
  setGlassFrost: (frost: number) => void;
  toggleSection: (section: SidebarSection) => void;
  /** Applies settings from Rust without saving them back. */
  hydrate: (settings: Settings) => void;
};

const root = () => document.documentElement;
const SAVE_DELAY_MS = 300;
let saveTimer: ReturnType<typeof setTimeout> | undefined;

function applyTheme(theme: Theme, accentSource: Rgb | null) {
  root().dataset["theme"] = theme;
  applyAccent(accentSource, theme);
}

function applyFrost(frost: number) {
  root().style.setProperty("--glass-frost", String(frost));
}

export const useUiStore = create<UiState>((set, get) => {
  /** Debounced save of the UI-owned fields, merged into the last known settings. */
  const persist = () => {
    if (!isTauri()) return;
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      const { settings, theme, glassFrost, sidebarCollapsed } = get();
      if (!settings) return;
      const next: Settings = { ...settings, theme, glassFrost, sidebarCollapsed };
      commands
        .settingsSet(next)
        .then((saved) => set({ settings: saved }))
        .catch(() => {
          // Keep the in-memory choice; the next change retries the save.
        });
    }, SAVE_DELAY_MS);
  };

  return {
    theme: "dark",
    accentSource: null,
    glassFrost: 0.5,
    sidebarCollapsed: [],
    settings: null,
    setTheme: (theme) => {
      applyTheme(theme, get().accentSource);
      set({ theme });
      persist();
    },
    setAccentSource: (accentSource) => {
      applyAccent(accentSource, get().theme);
      set({ accentSource });
    },
    setGlassFrost: (frost) => {
      const glassFrost = Math.min(Math.max(frost, 0), 1);
      applyFrost(glassFrost);
      set({ glassFrost });
      persist();
    },
    toggleSection: (section) => {
      const current = get().sidebarCollapsed;
      const sidebarCollapsed = current.includes(section)
        ? current.filter((s) => s !== section)
        : [...current, section];
      set({ sidebarCollapsed });
      persist();
    },
    hydrate: (settings) => {
      const glassFrost = settings.glassFrost ?? 0.5;
      applyTheme(settings.theme, get().accentSource);
      applyFrost(glassFrost);
      set({
        settings,
        theme: settings.theme,
        glassFrost,
        sidebarCollapsed: settings.sidebarCollapsed,
      });
    },
  };
});

const initial = useUiStore.getState();
applyTheme(initial.theme, initial.accentSource);

/** Loads settings from Rust before first paint (fast: local DB). Falls back to defaults. */
export async function loadSettings(timeoutMs = 1500): Promise<void> {
  if (!isTauri()) return;
  const timeout = new Promise<null>((resolve) => setTimeout(() => resolve(null), timeoutMs));
  try {
    const settings = await Promise.race([commands.settingsGet(), timeout]);
    if (settings) useUiStore.getState().hydrate(settings);
  } catch {
    // Defaults stay; Settings → About shows DB errors.
  }
}
