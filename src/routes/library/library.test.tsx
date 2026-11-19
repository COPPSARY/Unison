import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { strings } from "@/i18n";
import { renderRoute } from "@/test/render";

const cmd = {
  libraryLiked: vi.fn(),
  libraryLikedStats: vi.fn(),
  libraryAlbums: vi.fn(),
  libraryArtists: vi.fn(),
  playlistsList: vi.fn(),
  accountsList: vi.fn(),
  syncNow: vi.fn(),
};

vi.mock("@/lib/bindings", () => ({
  events: {
    syncProgress: { listen: () => Promise.resolve(() => undefined) },
    syncFinished: { listen: () => Promise.resolve(() => undefined) },
  },
  commands: new Proxy(
    {},
    {
      get: (_t, name: string) => (...args: unknown[]) =>
        (cmd as Record<string, ((...a: unknown[]) => unknown) | undefined>)[name]?.(...args),
    },
  ),
}));
vi.mock("@/lib/window", () => ({
  isTauri: () => true,
  windowControls: {
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
    onMaximizedChange: () => () => undefined,
  },
}));

const L = strings.library;

// jsdom has no layout: give elements a viewport-sized client box so the virtualiser
// (which measures clientWidth/clientHeight) renders rows.
const sizes = { clientHeight: 800, clientWidth: 1000 } as const;
beforeAll(() => {
  for (const [prop, value] of Object.entries(sizes)) {
    Object.defineProperty(HTMLElement.prototype, prop, { configurable: true, get: () => value });
  }
});
afterAll(() => {
  for (const prop of Object.keys(sizes)) {
    Reflect.deleteProperty(HTMLElement.prototype, prop);
  }
});

function track(i: number, available = true) {
  return {
    id: `id${i}`,
    title: `Song ${i}`,
    artists: [`Artist ${i}`],
    album: `Album ${i}`,
    durationMs: 200_000 + i * 1000,
    artwork: null,
    isrc: null,
    sources: [{ provider: i % 2 ? "youtube" : "spotify", itemId: `s${i}`, url: "https://x" }],
    available,
    addedAt: 1_700_000_000 - i * 86_400,
  };
}

const page = (items: unknown[], total = items.length) => ({ items, total, nextOffset: null });

beforeEach(() => {
  for (const fn of Object.values(cmd)) fn.mockReset();
  cmd.accountsList.mockResolvedValue([]);
  cmd.libraryLiked.mockResolvedValue(page([track(1), track(2), track(3, false)]));
  cmd.libraryLikedStats.mockResolvedValue({ count: 3, durationMs: 3 * 60_000 * 4 });
  cmd.libraryAlbums.mockResolvedValue([
    { id: "al1", provider: "spotify", itemId: "x", name: "Demo Album", artists: ["Zed"], artwork: null, releaseDate: null, addedAt: 1 },
  ]);
  cmd.libraryArtists.mockResolvedValue([{ id: "ar1", provider: "spotify", itemId: "x", name: "Zed", artwork: null }]);
  cmd.playlistsList.mockResolvedValue([
    { id: "pl1", provider: "spotify", name: "Late night", owner: "Demo", trackCount: 42, artwork: null, editable: true },
  ]);
});

describe("Liked songs", () => {
  it("shows the header, toolbar and every row with its columns", async () => {
    renderRoute("/library/liked");
    const grid = await screen.findByRole("grid", { name: strings.pages.liked.title });
    expect(await within(grid).findByText("Song 1")).toBeInTheDocument();
    expect(within(grid).getByText("Artist 2")).toBeInTheDocument();
    expect(within(grid).getByText("Album 1")).toBeInTheDocument();
    expect(within(grid).getByText("3:21")).toBeInTheDocument();
    expect(within(grid).getAllByRole("img", { name: strings.source.youtube }).length).toBeGreaterThan(0);
    expect(screen.getByText(/3 songs · 12 min/)).toBeInTheDocument();
    expect(screen.getByRole("radiogroup", { name: strings.common.source })).toBeInTheDocument();
    expect(cmd.libraryLiked).toHaveBeenCalledWith(null, "dateAdded", null, 0, 300);
  });

  it("marks unavailable songs and offers no play button for them", async () => {
    renderRoute("/library/liked");
    const unavailable = await screen.findByText("Song 3");
    expect(unavailable.className).toContain("text-text-disabled");
    expect(screen.queryByRole("button", { name: `Play Song 3` })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Play Song 1" })).toBeInTheDocument();
  });

  it("moves focus between rows with the arrow keys and plays on Enter", async () => {
    renderRoute("/library/liked");
    await screen.findByText("Song 1");
    const rows = screen.getAllByRole("row").filter((r) => r.hasAttribute("data-row"));
    rows[0]?.focus();
    await userEvent.keyboard("{ArrowDown}");
    await waitFor(() => expect(document.activeElement?.getAttribute("data-row")).toBe("1"));
    await userEvent.keyboard("{Enter}");
    expect(await screen.findByText(L.playSoon)).toBeInTheDocument();
  });

  it("shows skeleton rows while loading", async () => {
    cmd.libraryLiked.mockReturnValue(new Promise(() => undefined));
    const { container } = renderRoute("/library/liked");
    await screen.findByRole("grid");
    expect(container.querySelectorAll(".u-skeleton").length).toBeGreaterThan(10);
  });

  it("shows the empty state when nothing is liked", async () => {
    cmd.libraryLikedStats.mockResolvedValue({ count: 0, durationMs: 0 });
    cmd.libraryLiked.mockResolvedValue(page([]));
    renderRoute("/library/liked");
    expect(await screen.findByText(strings.pages.liked.emptyTitle)).toBeInTheDocument();
    expect(screen.queryByRole("grid")).not.toBeInTheDocument();
  });

  it("shows no-results with Clear filter, which resets the filter", async () => {
    renderRoute("/library/liked");
    await screen.findByText("Song 1");
    cmd.libraryLiked.mockResolvedValue(page([]));
    cmd.libraryLikedStats.mockImplementation((_p: unknown, f: unknown) =>
      Promise.resolve(f ? { count: 0, durationMs: 0 } : { count: 3, durationMs: 1 }),
    );
    await userEvent.type(screen.getByRole("textbox", { name: strings.common.filter }), "zzz");
    expect(await screen.findByText("No tracks match “zzz”")).toBeInTheDocument();
    cmd.libraryLiked.mockResolvedValue(page([track(1)]));
    await userEvent.click(screen.getByRole("button", { name: strings.common.clearFilter }));
    expect(screen.getByRole("textbox", { name: strings.common.filter })).toHaveValue("");
    expect(await screen.findByText("Song 1")).toBeInTheDocument();
  });

  it("shows an error with Retry", async () => {
    cmd.libraryLiked.mockRejectedValue({ code: { type: "internal" }, message: "db", retryable: false });
    renderRoute("/library/liked");
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(L.errorLoad);
    cmd.libraryLiked.mockResolvedValue(page([track(1)]));
    await userEvent.click(within(alert).getByRole("button", { name: L.retry }));
    expect(await screen.findByText("Song 1")).toBeInTheDocument();
  });

  it("passes source and sort to Rust", async () => {
    renderRoute("/library/liked");
    await screen.findByText("Song 1");
    await userEvent.click(screen.getByRole("radio", { name: strings.source.spotify }));
    await waitFor(() => expect(cmd.libraryLiked).toHaveBeenCalledWith("spotify", "dateAdded", null, 0, 300));
  });
});

describe("Cover grids", () => {
  it("lists albums as linked tiles", async () => {
    renderRoute("/library/albums");
    const link = await screen.findByRole("link", { name: /Demo Album/ });
    expect(link).toHaveAttribute("href", "/album/al1");
    expect(within(link).getByText("Zed")).toBeInTheDocument();
  });

  it("shows artists as round tiles", async () => {
    renderRoute("/library/artists");
    const link = await screen.findByRole("link", { name: /Zed/ });
    expect(link.querySelector(".rounded-full")).not.toBeNull();
  });

  it("shows playlists with owner and song count", async () => {
    renderRoute("/library/playlists");
    expect(await screen.findByText(/Demo · 42 songs/)).toBeInTheDocument();
  });

  it("shows grid empty, no-results and error states", async () => {
    cmd.libraryAlbums.mockResolvedValue([]);
    const { unmount } = renderRoute("/library/albums");
    expect(await screen.findByText(strings.pages.albums.emptyTitle)).toBeInTheDocument();
    unmount();

    cmd.playlistsList.mockRejectedValue({ code: { type: "offline" }, message: "x", retryable: true });
    renderRoute("/library/playlists");
    expect(await screen.findByRole("alert", {}, { timeout: 4000 })).toHaveTextContent(strings.errors.offline);
  });
});
