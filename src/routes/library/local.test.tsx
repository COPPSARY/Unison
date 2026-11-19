import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { format, strings } from "@/i18n";
import { onImportProgress } from "@/lib/local";
import { useLocalImportStore } from "@/stores/localImport";
import { useToastStore } from "@/stores/toast";
import { renderRoute } from "@/test/render";

const cmd = {
  libraryLocal: vi.fn(),
  libraryLocalStats: vi.fn(),
  localAddFiles: vi.fn(),
  localAddFolder: vi.fn(),
  accountsList: vi.fn(),
  playlistsList: vi.fn(),
  playerPlay: vi.fn(),
  playerState: vi.fn(),
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
  windowControls: {
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
    onMaximizedChange: () => () => undefined,
  },
}));

const t = strings.pages.local;
const r = strings.localImport;

const sizes = { clientHeight: 800, clientWidth: 1000 } as const;
beforeAll(() => {
  for (const [prop, value] of Object.entries(sizes)) {
    Object.defineProperty(HTMLElement.prototype, prop, { configurable: true, get: () => value });
  }
});
afterAll(() => {
  for (const prop of Object.keys(sizes)) Reflect.deleteProperty(HTMLElement.prototype, prop);
});

function track(i: number) {
  return {
    id: `id${i}`,
    title: `Local ${i}`,
    artists: [`Artist ${i}`],
    album: null,
    durationMs: 180_000,
    artwork: null,
    isrc: null,
    sources: [{ provider: "local", itemId: `fp${i}`, url: `/music/${i}.mp3` }],
    available: true,
    addedAt: 1_700_000_000,
  };
}

const page = (items: unknown[], total = items.length) => ({ items, total, nextOffset: null });

const progress = (over: Partial<Parameters<typeof onImportProgress>[0]> = {}) => ({
  jobId: 1,
  scanned: 0,
  total: null,
  imported: 0,
  skipped: 0,
  errors: [],
  finished: false,
  error: null,
  ...over,
});

beforeEach(() => {
  for (const fn of Object.values(cmd)) fn.mockReset();
  cmd.accountsList.mockResolvedValue([]);
  cmd.playlistsList.mockResolvedValue([]);
  cmd.libraryLocal.mockResolvedValue(page([track(1), track(2)]));
  cmd.libraryLocalStats.mockResolvedValue({ count: 2, durationMs: 360_000 });
  cmd.localAddFiles.mockResolvedValue(null);
  cmd.localAddFolder.mockResolvedValue(null);
  useLocalImportStore.setState({ dragging: false, job: null, issues: [], issuesOpen: false });
  useToastStore.setState({ toasts: [] });
});

describe("Local files page", () => {
  it("lists local songs with the This PC mark, count and add buttons", async () => {
    renderRoute("/library/local");
    const grid = await screen.findByRole("grid", { name: t.title });
    expect(await within(grid).findByText("Local 1")).toBeInTheDocument();
    expect(within(grid).getAllByRole("img", { name: strings.source.local }).length).toBe(2);
    expect(screen.getByText(/2 songs · 6 min/)).toBeInTheDocument();
    // One source only: no source filter.
    expect(screen.queryByRole("radiogroup", { name: strings.common.source })).not.toBeInTheDocument();
    expect(cmd.libraryLocal).toHaveBeenCalledWith("dateAdded", null, 0, 300);
    expect(screen.getByRole("link", { name: strings.nav.local })).toBeInTheDocument();
  });

  it("opens the native dialogs from the header", async () => {
    renderRoute("/library/local");
    await screen.findByText("Local 1");
    await userEvent.click(screen.getByRole("button", { name: t.addFiles }));
    expect(cmd.localAddFiles).toHaveBeenCalledWith(t.filesDialogTitle, t.audioFilter);
    await userEvent.click(screen.getByRole("button", { name: t.addFolder }));
    expect(cmd.localAddFolder).toHaveBeenCalledWith(t.folderDialogTitle);
  });

  it("shows the empty state with add buttons", async () => {
    cmd.libraryLocalStats.mockResolvedValue({ count: 0, durationMs: 0 });
    cmd.libraryLocal.mockResolvedValue(page([]));
    renderRoute("/library/local");
    expect(await screen.findByText(t.emptyTitle)).toBeInTheDocument();
    expect(screen.getByText(t.emptyBody)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: t.addFolder }));
    expect(cmd.localAddFolder).toHaveBeenCalledOnce();
  });

  it("shows import progress while files are being added", async () => {
    cmd.libraryLocalStats.mockResolvedValue({ count: 0, durationMs: 0 });
    cmd.libraryLocal.mockResolvedValue(page([]));
    renderRoute("/library/local");
    await screen.findByText(t.emptyTitle);
    act(() => onImportProgress(progress({ scanned: 0, total: null })));
    expect(screen.getByText(t.scanning)).toBeInTheDocument();
    act(() => onImportProgress(progress({ scanned: 200, total: 1000 })));
    expect(screen.getByText(format(t.importing, { done: "200", total: "1,000" }))).toBeInTheDocument();
  });

  it("shows skeleton rows while loading", async () => {
    cmd.libraryLocal.mockReturnValue(new Promise(() => undefined));
    const { container } = renderRoute("/library/local");
    await screen.findByRole("grid");
    expect(container.querySelectorAll(".u-skeleton").length).toBeGreaterThan(10);
  });

  it("shows no-results with Clear filter", async () => {
    renderRoute("/library/local");
    await screen.findByText("Local 1");
    cmd.libraryLocal.mockResolvedValue(page([]));
    cmd.libraryLocalStats.mockImplementation((filter: string | null) =>
      Promise.resolve(filter ? { count: 0, durationMs: 0 } : { count: 2, durationMs: 360_000 }),
    );
    await userEvent.type(screen.getByRole("textbox", { name: strings.common.filter }), "zzz");
    expect(await screen.findByText(format(strings.library.noResults, { query: "zzz" }))).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: strings.common.clearFilter }));
    expect(screen.getByRole("textbox", { name: strings.common.filter })).toHaveValue("");
  });

  it("shows an error with Retry", async () => {
    cmd.libraryLocal.mockRejectedValue({ code: { type: "internal" }, message: "x", retryable: false });
    renderRoute("/library/local");
    expect(await screen.findByText(strings.library.errorLoad)).toBeInTheDocument();
    cmd.libraryLocal.mockResolvedValue(page([track(1)]));
    await userEvent.click(screen.getByRole("button", { name: strings.library.retry }));
    expect(await screen.findByText("Local 1")).toBeInTheDocument();
  });
});

describe("Drop overlay", () => {
  it("appears while files are dragged over the window", async () => {
    renderRoute("/library/local");
    await screen.findByText("Local 1");
    expect(screen.queryByTestId("drop-overlay")).not.toBeInTheDocument();
    act(() => useLocalImportStore.getState().setDragging(true));
    expect(screen.getByTestId("drop-overlay")).toHaveTextContent(r.dropTitle);
    expect(screen.getByTestId("drop-overlay")).toHaveTextContent(r.dropReference);
    act(() => useLocalImportStore.getState().setDragging(false));
    expect(screen.queryByTestId("drop-overlay")).not.toBeInTheDocument();
  });
});

describe("Import summary", () => {
  const toasts = () => useToastStore.getState().toasts;

  it("reports added songs and per-file reasons behind Details", async () => {
    renderRoute("/library/local");
    await screen.findByText("Local 1");
    act(() =>
      onImportProgress(
        progress({
          finished: true,
          imported: 3,
          errors: [
            { path: "/drop/bought.m4p", reason: "drmProtected" },
            { path: "C:\\Music\\voice.opus", reason: "unsupportedFormat" },
          ],
        }),
      ),
    );
    expect(toasts()[0]?.title).toBe(format(r.added, { count: "3" }));
    expect(toasts()[0]?.description).toBe(format(r.issues, { count: "2" }));
    await userEvent.click(await screen.findByRole("button", { name: r.details }));
    const dialog = await screen.findByRole("dialog", { name: r.detailsTitle });
    expect(within(dialog).getByText("bought.m4p")).toBeInTheDocument();
    expect(within(dialog).getByText(r.reason.drmProtected)).toBeInTheDocument();
    expect(within(dialog).getByText("voice.opus")).toBeInTheDocument();
    expect(within(dialog).getByText(r.reason.unsupportedFormat)).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: strings.common.ok }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("covers only-failures, nothing new, nothing found and a stopped job", () => {
    onImportProgress(progress({ finished: true, errors: [{ path: "/x.mp3", reason: "unreadable" }] }));
    expect(toasts().at(-1)).toMatchObject({ title: r.issuesOne, tone: "danger" });
    onImportProgress(progress({ finished: true, skipped: 4 }));
    expect(toasts().at(-1)).toMatchObject({ title: r.nothingNew });
    onImportProgress(progress({ finished: true }));
    expect(toasts().at(-1)).toMatchObject({ title: r.nothingFound });
    onImportProgress(
      progress({ finished: true, error: { code: { type: "internal" }, message: "db", retryable: false } }),
    );
    expect(toasts().at(-1)).toMatchObject({ title: r.failed, tone: "danger" });
    expect(useLocalImportStore.getState().job).toBeNull();
  });

  it("uses the singular for one song", () => {
    onImportProgress(progress({ finished: true, imported: 1 }));
    expect(toasts().at(-1)?.title).toBe(r.addedOne);
  });
});
