import { act, fireEvent, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { strings } from "@/i18n";
import { usePlayerStore } from "@/stores/player";
import { useUiStore } from "@/stores/ui";
import { media } from "@/test/setup";
import { renderRoute } from "@/test/render";

vi.mock("@/lib/window", () => ({
  isTauri: () => false,
  windowControls: {
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
    onMaximizedChange: () => () => undefined,
  },
}));

const { windowControls } = await import("@/lib/window");

const heading = (name: string) => screen.findByRole("heading", { level: 1, name });

const ROUTES: Array<[string, string]> = [
  ["/", strings.pages.home.title],
  ["/search", strings.pages.search.title],
  ["/library/liked", strings.pages.liked.title],
  ["/library/albums", strings.pages.albums.title],
  ["/library/playlists", strings.pages.playlists.title],
  ["/library/artists", strings.pages.artists.title],
  ["/playlist/abc", strings.pages.playlist.title],
  ["/album/abc", strings.pages.album.title],
  ["/artist/abc", strings.pages.artist.title],
  ["/settings", strings.pages.settings.title],
  ["/extensions", strings.extensions.title],
];

beforeEach(() => {
  media.matches = false;
  usePlayerStore.setState({ state: null });
  useUiStore.setState({ sidebarCollapsed: [] });
});

describe("shell routes", () => {
  it.each(ROUTES)("%s renders its page inside the shell", async (path, title) => {
    renderRoute(path);
    expect(await heading(title)).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: strings.nav.sidebar })).toBeInTheDocument();
    expect(screen.getByRole("contentinfo", { name: strings.player.playerBar })).toBeInTheDocument();
  });

  it("marks the active sidebar item and navigates on click", async () => {
    renderRoute("/");
    const home = screen.getByRole("link", { name: strings.nav.home });
    expect(home).toHaveAttribute("aria-current", "page");
    await userEvent.click(screen.getByRole("link", { name: strings.nav.albums }));
    expect(await heading(strings.pages.albums.title)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: strings.nav.albums })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(home).not.toHaveAttribute("aria-current");
  });

  it("reaches every sidebar destination by keyboard", async () => {
    renderRoute("/");
    const links = screen
      .getByRole("navigation", { name: strings.nav.sidebar })
      .querySelectorAll("a");
    expect(links.length).toBeGreaterThanOrEqual(7);
    for (const link of links) {
      link.focus();
      await userEvent.keyboard("{Enter}");
      expect(link).toHaveAttribute("aria-current", "page");
    }
  });
});

describe("shortcuts", () => {
  it("Ctrl+K, Ctrl+L and Ctrl+, navigate", async () => {
    renderRoute("/");
    fireEvent.keyDown(window, { key: "k", ctrlKey: true });
    expect(await heading(strings.pages.search.title)).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: strings.pages.search.title })).toHaveFocus();
    fireEvent.keyDown(window, { key: "l", ctrlKey: true });
    expect(await heading(strings.pages.liked.title)).toBeInTheDocument();
    fireEvent.keyDown(window, { key: ",", ctrlKey: true });
    expect(await heading(strings.pages.settings.title)).toBeInTheDocument();
  });

  it("Alt+Left and Alt+Right move through history", async () => {
    renderRoute("/");
    fireEvent.keyDown(window, { key: "l", ctrlKey: true });
    await heading(strings.pages.liked.title);
    fireEvent.keyDown(window, { key: "ArrowLeft", altKey: true });
    expect(await heading(strings.pages.home.title)).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "ArrowRight", altKey: true });
    expect(await heading(strings.pages.liked.title)).toBeInTheDocument();
  });

  it("Space toggles playback but not while typing", async () => {
    usePlayerStore.setState({ state: fakePlayerState("paused") });
    renderRoute("/search");
    await heading(strings.pages.search.title);
    fireEvent.keyDown(screen.getByRole("textbox"), { key: " " });
    expect(usePlayerStore.getState().state?.status).toBe("paused");
    act(() => {
      (document.activeElement as HTMLElement | null)?.blur();
    });
    fireEvent.keyDown(document.body, { key: " " });
    expect(usePlayerStore.getState().state?.status).toBe("playing");
  });
});

describe("sidebar sections", () => {
  it("collapse and expand from their header (Finder-style)", async () => {
    renderRoute("/");
    await heading(strings.pages.home.title);
    const header = screen.getByRole("button", { name: strings.nav.library });
    expect(header).toHaveAttribute("aria-expanded", "true");
    await userEvent.click(header);
    expect(header).toHaveAttribute("aria-expanded", "false");
    expect(screen.queryByRole("link", { name: strings.nav.albums })).not.toBeInTheDocument();
    expect(useUiStore.getState().sidebarCollapsed).toEqual(["library"]);
    await userEvent.click(header);
    expect(screen.getByRole("link", { name: strings.nav.albums })).toBeInTheDocument();
  });

  it("tints item icons with the accent and marks selection with a fill, not a bar", async () => {
    renderRoute("/library/liked");
    const link = await screen.findByRole("link", { name: strings.nav.liked });
    expect(link.querySelector("svg")?.getAttribute("class")).toContain("text-accent");
    expect(link.className).toContain("bg-fill-secondary");
  });
});

describe("sidebar collapse", () => {
  it("shows icons with accessible names and hides group labels below 1100 px", async () => {
    media.matches = true;
    renderRoute("/");
    await heading(strings.pages.home.title);
    expect(screen.queryByText(strings.nav.yourPlaylists)).not.toBeInTheDocument();
    expect(screen.getByRole("link", { name: strings.nav.liked })).toBeInTheDocument();
    expect(screen.queryByText(strings.nav.liked, { selector: "span" })).not.toBeInTheDocument();
  });
});

describe("title bar", () => {
  it("drives the window controls", async () => {
    renderRoute("/");
    await userEvent.click(screen.getByRole("button", { name: strings.window.minimize }));
    await userEvent.click(screen.getByRole("button", { name: strings.window.maximize }));
    await userEvent.click(screen.getByRole("button", { name: strings.window.close }));
    expect(windowControls.minimize).toHaveBeenCalledOnce();
    expect(windowControls.toggleMaximize).toHaveBeenCalledOnce();
    expect(windowControls.close).toHaveBeenCalledOnce();
  });
});

describe("player bar placeholder", () => {
  it("shows Not playing with disabled controls", async () => {
    renderRoute("/");
    await heading(strings.pages.home.title);
    expect(screen.getByText(strings.player.notPlaying)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: strings.player.play })).toBeDisabled();
  });
});

function fakePlayerState(status: "playing" | "paused") {
  return {
    status,
    trackId: "t1",
    source: { kind: "local" as const, itemId: "f", url: "C:/a.mp3" },
    volume: 80,
    repeat: "off" as const,
    shuffle: false,
    queueLength: 1,
    track: { title: "Song", artists: ["A"], album: null, artwork: null },
    positionMs: 0,
    durationMs: 1000,
    canSeek: true,
  };
}
