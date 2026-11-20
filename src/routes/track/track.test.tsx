import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { format, strings } from "@/i18n";
import { renderRoute } from "@/test/render";

const cmd = {
  playlistsList: vi.fn(),
  extensionSong: vi.fn(),
  extensionPlaySong: vi.fn(),
  extensionLikeSong: vi.fn(),
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

const t = strings.pages.track;

const detail = (liked = false) => ({
  song: {
    key: "a:1",
    title: "Song One",
    artists: ["Artist"],
    album: "LP",
    durationMs: 200_000,
    artwork: null,
    isrc: "USABC2400001",
    sources: [
      { key: "a:1", extensionId: "a", extensionName: "Store A", icon: null, canPlay: true, qualities: [], quality: null, url: "" },
      {
        key: "b:9", extensionId: "b", extensionName: "Store B", icon: null, canPlay: true,
        qualities: [{ id: "LOSSLESS", label: "Lossless", description: null }, { id: "HI_RES", label: "Hi-Res", description: null }],
        quality: "LOSSLESS", url: "",
      },
      { key: "c:3", extensionId: "c", extensionName: "Downloader", icon: null, canPlay: false, qualities: [], quality: null, url: "" },
    ],
  },
  liked,
  trackId: null,
});

beforeEach(() => {
  for (const fn of Object.values(cmd)) fn.mockReset();
  cmd.playlistsList.mockResolvedValue([]);
  cmd.extensionSong.mockResolvedValue(detail());
  cmd.extensionPlaySong.mockResolvedValue(null);
});

describe("Track page", () => {
  it("lists sources with qualities and plays from the chosen one", async () => {
    renderRoute("/track/a%3A1");
    expect(await screen.findByRole("heading", { level: 1, name: "Song One" })).toBeInTheDocument();
    expect(cmd.extensionSong).toHaveBeenCalledWith("a:1");
    const sources = screen.getByRole("list", { name: t.sources });
    expect(within(sources).getAllByRole("listitem")).toHaveLength(3);
    expect(within(sources).getByText(t.cantPlay)).toBeInTheDocument();
    expect(within(sources).getByRole("radio", { name: format(t.playFrom, { name: "Downloader" }) })).toBeDisabled();
    expect(within(sources).getByRole("radio", { name: format(t.playFrom, { name: "Store A" }) })).toBeChecked();

    await userEvent.click(within(sources).getByRole("radio", { name: format(t.playFrom, { name: "Store B" }) }));
    expect(within(sources).getByRole("combobox", { name: format(t.quality, { name: "Store B" }) })).toHaveTextContent("Lossless");
    await userEvent.click(within(screen.getByRole("main")).getByRole("button", { name: t.play }));
    await waitFor(() => expect(cmd.extensionPlaySong).toHaveBeenCalledWith("a:1", "b:9", "LOSSLESS"));
  });

  it("likes the song", async () => {
    cmd.extensionLikeSong.mockResolvedValue(true);
    renderRoute("/track/a%3A1");
    await userEvent.click(await screen.findByRole("button", { name: t.like }));
    expect(cmd.extensionLikeSong).toHaveBeenCalledWith("a:1", true);
    expect(await screen.findByText(t.liked)).toBeInTheDocument();
  });

  it("shows details in their own tab", async () => {
    renderRoute("/track/a%3A1");
    await userEvent.click(await screen.findByRole("tab", { name: t.details }));
    const panel = screen.getByRole("tabpanel");
    expect(within(panel).getByText("USABC2400001")).toBeInTheDocument();
    expect(within(panel).getByText("Store A, Store B, Downloader")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: t.details })).toHaveAttribute("aria-selected", "true");
  });

  it("says when a song can't be found", async () => {
    cmd.extensionSong.mockRejectedValue({ code: { type: "notFound" }, message: "x", retryable: false });
    renderRoute("/track/zz%3A1");
    expect(await screen.findByText(t.notFoundTitle)).toBeInTheDocument();
  });
});
