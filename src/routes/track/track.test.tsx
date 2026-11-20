import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { format, strings } from "@/i18n";
import { useDownloadsStore } from "@/stores/downloads";
import { renderRoute } from "@/test/render";

const cmd = {
  playlistsList: vi.fn(),
  extensionSong: vi.fn(),
  extensionPlaySong: vi.fn(),
  extensionLikeSong: vi.fn(),
  extensionDownload: vi.fn(),
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
      {
        key: "a:1", extensionId: "a", extensionName: "Store A", icon: null, canPlay: true, qualities: [], quality: null, url: "",
        license: { name: "CC BY-SA 4.0", url: "https://creativecommons.org/licenses/by-sa/4.0/" },
      },
      {
        key: "b:9", extensionId: "b", extensionName: "Store B", icon: null, canPlay: true,
        qualities: [{ id: "LOSSLESS", label: "Lossless", description: null }, { id: "HI_RES", label: "Hi-Res", description: null }],
        quality: "LOSSLESS", url: "", license: null,
      },
      { key: "c:3", extensionId: "c", extensionName: "Downloader", icon: null, canPlay: false, qualities: [], quality: null, url: "", license: null },
    ],
  },
  liked,
  trackId: null,
});

beforeEach(() => {
  for (const fn of Object.values(cmd)) fn.mockReset();
  useDownloadsStore.setState({ jobs: {} });
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
    await waitFor(() => expect(cmd.extensionPlaySong).toHaveBeenCalledWith("a:1", "b:9", null));
    await userEvent.click(within(sources).getByRole("combobox", { name: format(t.quality, { name: "Store B" }) }));
    await userEvent.click(await screen.findByRole("option", { name: "Hi-Res" }));
    await userEvent.click(within(screen.getByRole("main")).getByRole("button", { name: t.play }));
    await waitFor(() => expect(cmd.extensionPlaySong).toHaveBeenLastCalledWith("a:1", "b:9", "HI_RES"));
  });

  it("offers Download only for a source with an open licence", async () => {
    cmd.extensionDownload.mockResolvedValue(4);
    renderRoute("/track/a%3A1");
    const sources = await screen.findByRole("list", { name: t.sources });
    expect(within(sources).getByText(format(t.license, { name: "CC BY-SA 4.0" }))).toBeInTheDocument();
    expect(within(sources).queryByRole("button", { name: format(t.downloadFrom, { name: "Store B" }) })).toBeNull();
    expect(within(sources).queryByRole("button", { name: format(t.downloadFrom, { name: "Downloader" }) })).toBeNull();
    const download = within(sources).getByRole("button", { name: format(t.downloadFrom, { name: "Store A" }) });
    await userEvent.click(download);
    expect(cmd.extensionDownload).toHaveBeenCalledWith("a:1", "a:1");
    await waitFor(() => expect(download).toBeDisabled());
    expect(download).toHaveTextContent(t.downloadingUnknown);
  });

  it("says why a download was refused", async () => {
    cmd.extensionDownload.mockRejectedValue({ code: { type: "downloadNotAllowed" }, message: "x", retryable: false });
    renderRoute("/track/a%3A1");
    await userEvent.click(await screen.findByRole("button", { name: format(t.downloadFrom, { name: "Store A" }) }));
    expect(await screen.findByText(strings.errors.downloadNotAllowed)).toBeInTheDocument();
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
