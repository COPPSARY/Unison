import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { format, strings } from "@/i18n";
import { renderRoute } from "@/test/render";

const cmd = {
  playlistsList: vi.fn(),
  extensionList: vi.fn(),
  extensionSearchAll: vi.fn(),
  extensionPlaySong: vi.fn(),
  playerState: vi.fn(),
};

vi.mock("@/lib/bindings", () => ({
  events: new Proxy({}, { get: () => ({ listen: () => Promise.resolve(() => undefined) }) }),
  commands: new Proxy(
    {},
    { get: (_t, name: string) => (...args: unknown[]) => (cmd as Record<string, (...a: unknown[]) => unknown>)[name]?.(...args) },
  ),
}));
vi.mock("@/lib/window", () => ({
  isTauri: () => true,
  windowControls: { minimize: vi.fn(), toggleMaximize: vi.fn(), close: vi.fn(), onMaximizedChange: () => () => undefined },
}));

const s = strings.pages.search;

const installed = (enabled = true) => ({
  id: "radio", name: "Radio", version: "1.0.0", author: null, description: null, hosts: [], repoUrl: "x",
  commitSha: "0123456789abcdef0123456789abcdef01234567", enabled, installedAt: 1, runsCode: true,
  capabilities: ["search", "play"] as ("search" | "play")[], types: [], homepage: null, icon: null,
});

const source = (ext: string, key: string, canPlay = true) => ({
  key, extensionId: ext, extensionName: ext.toUpperCase(), icon: null, canPlay, qualities: [], quality: null, url: "",
});

const song = (key: string, title: string, sources: ReturnType<typeof source>[]) => ({
  key, title, artists: ["Someone"], album: "LP", durationMs: 61_000, artwork: null, isrc: null, sources,
});

beforeEach(() => {
  for (const fn of Object.values(cmd)) fn.mockReset();
  cmd.playlistsList.mockResolvedValue([]);
  cmd.extensionList.mockResolvedValue([installed()]);
  cmd.extensionPlaySong.mockResolvedValue(null);
});

describe("Search", () => {
  it("shows one merged song list, not a section per extension", async () => {
    cmd.extensionSearchAll.mockResolvedValue({
      songs: [
        song("a:1", "Song One", [source("a", "a:1"), source("b", "b:9")]),
        song("b:2", "Song Two", [source("b", "b:2")]),
      ],
      failures: [{ extensionId: "c", extensionName: "C", error: { code: { type: "offline" }, message: "x", retryable: true } }],
    });
    renderRoute("/search");
    await userEvent.type(screen.getByLabelText(s.title), "song");
    const list = await screen.findByRole("list", { name: s.songs }, { timeout: 2000 });
    expect(cmd.extensionSearchAll).toHaveBeenCalledWith("song");
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    expect(within(list).getByText(format(s.sourceCount, { count: 2 }))).toBeInTheDocument();
    expect(within(list).getByText(s.sourceCountOne)).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "A" })).not.toBeInTheDocument();
    expect(screen.getByRole("alert")).toHaveTextContent(
      format(s.failed, { name: "C", reason: strings.errors.offline }),
    );
  });

  it("opens a song's page on click and plays from the artwork button", async () => {
    cmd.extensionSearchAll.mockResolvedValue({
      songs: [song("a:1", "Song One", [source("a", "a:1", false), source("b", "b:9")])],
      failures: [],
    });
    renderRoute("/search");
    await userEvent.type(screen.getByLabelText(s.title), "song");
    const open = await screen.findByRole("link", { name: format(s.openSong, { title: "Song One" }) }, { timeout: 2000 });
    expect(open).toHaveAttribute("href", "/track/a%3A1");
    await userEvent.click(screen.getByRole("button", { name: format(s.playSong, { title: "Song One" }) }));
    await waitFor(() => expect(cmd.extensionPlaySong).toHaveBeenCalledWith("a:1", "b:9", null));
  });

  it("asks to add an extension when none is on", async () => {
    cmd.extensionList.mockResolvedValue([installed(false)]);
    renderRoute("/search");
    expect(await screen.findByText(s.noExtensions)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: s.noExtensionsAction })).toBeInTheDocument();
  });

  it("says when nothing matches", async () => {
    cmd.extensionSearchAll.mockResolvedValue({ songs: [], failures: [] });
    renderRoute("/search");
    await userEvent.type(screen.getByLabelText(s.title), "zzz");
    expect(await screen.findByText(format(s.noResults, { query: "zzz" }), {}, { timeout: 2000 })).toBeInTheDocument();
  });
});
