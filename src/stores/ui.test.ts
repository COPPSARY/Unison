import type { Settings_Serialize as Settings } from "@/lib/bindings";

const settingsGet = vi.fn();
const settingsSet = vi.fn();

vi.mock("@/lib/bindings", () => ({
  commands: {
    settingsGet: () => settingsGet(),
    settingsSet: (s: Settings) => settingsSet(s),
  },
}));
vi.mock("@/lib/window", () => ({ isTauri: () => true }));

const { useUiStore, loadSettings } = await import("./ui");

const saved: Settings = {
  theme: "light",
  glassFrost: 0.8,
  windowMaterial: "mica",
  sidebarCollapsed: ["library"],
  syncIntervalMin: 60,
  preferredSource: "youtube",
};

beforeEach(() => {
  vi.useFakeTimers();
  settingsGet.mockReset();
  settingsSet.mockReset();
  settingsSet.mockImplementation((s: Settings) => Promise.resolve(s));
});
afterEach(() => vi.useRealTimers());

describe("ui store", () => {
  it("hydrates from Rust settings and applies theme + frost", async () => {
    settingsGet.mockResolvedValue(saved);
    await loadSettings();
    const st = useUiStore.getState();
    expect(st.theme).toBe("light");
    expect(st.sidebarCollapsed).toEqual(["library"]);
    expect(document.documentElement.dataset["theme"]).toBe("light");
    expect(document.documentElement.style.getPropertyValue("--glass-frost")).toBe("0.8");
    expect(settingsSet).not.toHaveBeenCalled();
  });

  it("saves UI changes merged into the other settings, debounced", async () => {
    useUiStore.getState().hydrate(saved);
    useUiStore.getState().setGlassFrost(0.2);
    useUiStore.getState().setGlassFrost(0.3);
    useUiStore.getState().setTheme("dark");
    useUiStore.getState().toggleSection("listen");
    expect(settingsSet).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(400);
    expect(settingsSet).toHaveBeenCalledOnce();
    expect(settingsSet).toHaveBeenCalledWith({
      ...saved,
      theme: "dark",
      glassFrost: 0.3,
      sidebarCollapsed: ["library", "listen"],
    });
  });

  it("re-derives the accent for the theme and clamps frost", () => {
    const style = document.documentElement.style;
    useUiStore.getState().setTheme("light");
    useUiStore.getState().setAccentSource({ r: 196, g: 32, b: 54 });
    const light = style.getPropertyValue("--accent");
    useUiStore.getState().setTheme("dark");
    expect(style.getPropertyValue("--accent")).not.toBe(light);
    useUiStore.getState().setGlassFrost(3);
    expect(style.getPropertyValue("--glass-frost")).toBe("1");
  });

  it("keeps defaults when settings fail to load", async () => {
    useUiStore.setState({ theme: "dark", settings: null });
    settingsGet.mockRejectedValue({ code: { type: "internal" }, message: "x", retryable: false });
    await loadSettings();
    expect(useUiStore.getState().theme).toBe("dark");
  });
});
