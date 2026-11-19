import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { strings } from "@/i18n";
import { renderRoute } from "@/test/render";

const cmd = {
  playlistGet: vi.fn(),
  playlistTracks: vi.fn(),
  playlistOpenExternal: vi.fn(),
  accountsList: vi.fn(),
  playlistsList: vi.fn(),
};

vi.mock("@/lib/bindings", () => ({
  events: {
    syncProgress: { listen: () => Promise.resolve(() => undefined) },
    syncFinished: { listen: () => Promise.resolve(() => undefined) },
    localDrag: { listen: () => Promise.resolve(() => undefined) },
    localImportProgress: { listen: () => Promise.resolve(() => undefined) },
    localChanged: { listen: () => Promise.resolve(() => undefined) },
    playerState: { listen: () => Promise.resolve(() => undefined) },
    playerFallback: { listen: () => Promise.resolve(() => undefined) },
    playerYoutubeCommand: { listen: () => Promise.resolve(() => undefined) },
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
  windowControls: { minimize: vi.fn(), toggleMaximize: vi.fn(), close: vi.fn(), onMaximizedChange: () => () => undefined },
}));

const sizes = { clientHeight: 800, clientWidth: 1000 } as const;
beforeAll(() => {
  for (const [prop, value] of Object.entries(sizes)) {
    Object.defineProperty(HTMLElement.prototype, prop, { configurable: true, get: () => value });
  }
});
afterAll(() => {
  for (const prop of Object.keys(sizes)) Reflect.deleteProperty(HTMLElement.prototype, prop);
});

const t = strings.pages.playlist;

function detail(over: Record<string, unknown> = {}) {
  return {
    playlist: { id: "pl1", provider: "spotify", name: "Late night", owner: "Demo", trackCount: 2, artwork: null, editable: true },
    url: "https://open.spotify.com/playlist/x",
    itemsAvailable: true,
    durationMs: 400_000,
    ...over,
  };
}

const track = (i: number) => ({
  id: `t${i}`, title: `Track ${i}`, artists: ["A"], album: null, durationMs: 200_000, artwork: null,
  isrc: null, sources: [{ provider: "spotify", itemId: `s${i}`, url: "https://x" }], available: true, addedAt: 1,
});

beforeEach(() => {
  for (const fn of Object.values(cmd)) fn.mockReset();
  cmd.accountsList.mockResolvedValue([]);
  cmd.playlistsList.mockResolvedValue([]);
  cmd.playlistGet.mockResolvedValue(detail());
  cmd.playlistTracks.mockResolvedValue({ items: [track(1), track(2)], total: 2, nextOffset: null });
  cmd.playlistOpenExternal.mockResolvedValue(null);
});

describe("Playlist page", () => {
  it("shows an owned playlist's header and songs", async () => {
    renderRoute("/playlist/pl1");
    expect(await screen.findByRole("heading", { level: 1, name: "Late night" })).toBeInTheDocument();
    expect(screen.getByText(/Demo · 2 songs · 7 min/)).toBeInTheDocument();
    const grid = await screen.findByRole("grid", { name: "Late night" });
    expect(await within(grid).findByText("Track 2")).toBeInTheDocument();
    expect(cmd.playlistTracks).toHaveBeenCalledWith("pl1", 0, 300);
  });

  it("explains followed playlists, offers Open in Spotify and never asks for items", async () => {
    cmd.playlistGet.mockResolvedValue(
      detail({ itemsAvailable: false, durationMs: null, playlist: { ...detail().playlist, editable: false, owner: "Other", trackCount: 120 } }),
    );
    renderRoute("/playlist/pl1");
    expect(await screen.findByText(t.followedReason)).toBeInTheDocument();
    expect(screen.queryByRole("grid")).not.toBeInTheDocument();
    // The player bar has its own Play button; check only the page content.
    expect(within(screen.getByRole("main")).queryByRole("button", { name: strings.library.play })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Open in Spotify/ }));
    expect(cmd.playlistOpenExternal).toHaveBeenCalledWith("pl1");
    expect(cmd.playlistTracks).not.toHaveBeenCalled();
  });

  it("shows not found for an unknown playlist", async () => {
    cmd.playlistGet.mockRejectedValue({ code: { type: "notFound" }, message: "x", retryable: false });
    renderRoute("/playlist/nope");
    expect(await screen.findByText(t.emptyTitle)).toBeInTheDocument();
  });

  it("shows an empty playlist state", async () => {
    cmd.playlistGet.mockResolvedValue(detail({ playlist: { ...detail().playlist, trackCount: 0 }, durationMs: 0 }));
    cmd.playlistTracks.mockResolvedValue({ items: [], total: 0, nextOffset: null });
    renderRoute("/playlist/pl1");
    expect(await screen.findByText(t.emptyPlaylistTitle)).toBeInTheDocument();
  });

  it("shows a load error with Retry when items fail", async () => {
    cmd.playlistTracks.mockRejectedValue({ code: { type: "offline" }, message: "x", retryable: false });
    renderRoute("/playlist/pl1");
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(strings.errors.offline);
    cmd.playlistTracks.mockResolvedValue({ items: [track(1)], total: 1, nextOffset: null });
    await userEvent.click(within(alert).getByRole("button", { name: strings.library.retry }));
    await waitFor(() => expect(screen.getByText("Track 1")).toBeInTheDocument());
  });
});
