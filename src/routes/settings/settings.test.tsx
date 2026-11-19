import { act, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { strings } from "@/i18n";
import { renderRoute } from "@/test/render";
import { useUiStore } from "@/stores/ui";

const youtubeListeners: Array<(e: { payload: unknown }) => void> = [];

const cmd = {
  appInfo: vi.fn(),
  accountsList: vi.fn(),
  spotifyHasClientId: vi.fn(),
  spotifyRedirectUris: vi.fn(),
  spotifySetClientId: vi.fn(),
  accountConnect: vi.fn(),
  accountCancelConnect: vi.fn(),
  spotifyOpenDashboard: vi.fn(),
  syncNow: vi.fn(),
  youtubeTestPlayback: vi.fn(),
  youtubeTestReport: vi.fn(),
  playlistsList: vi.fn(),
  spotifyCapabilities: vi.fn(),
  playerOutputs: vi.fn(),
  playerSetOutput: vi.fn(),
  playerTestSound: vi.fn(),
  settingsGet: vi.fn(),
  settingsSet: vi.fn(),
  localCopyTarget: vi.fn(),
  localFoldersList: vi.fn(),
  localPickFolder: vi.fn(),
  localFolderAdd: vi.fn(),
  localFolderRemove: vi.fn(),
  localRescan: vi.fn(),
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
    playerYoutubeCommand: {
      listen: (cb: (e: { payload: unknown }) => void) => {
        youtubeListeners.push(cb);
        return Promise.resolve(() => undefined);
      },
    },
  },
  commands: new Proxy(
    {},
    { get: (_t, name: string) => (...args: unknown[]) => (cmd as Record<string, (...a: unknown[]) => unknown>)[name]?.(...args) },
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

const a = strings.accounts;
const GOOD_ID = "0123456789abcdef0123456789abcdef";
const notConnected = [
  { provider: "spotify", status: "notConnected", displayName: null, lastSyncedAt: null },
  { provider: "youtube", status: "notConnected", displayName: null, lastSyncedAt: null },
];

beforeEach(() => {
  for (const fn of Object.values(cmd)) fn.mockReset();
  cmd.appInfo.mockResolvedValue({ version: "0.1.0", schemaVersion: 1 });
  cmd.accountsList.mockResolvedValue(notConnected);
  cmd.spotifyHasClientId.mockResolvedValue(false);
  cmd.spotifyRedirectUris.mockResolvedValue([
    "http://127.0.0.1:43821/callback",
    "http://127.0.0.1:43822/callback",
    "http://127.0.0.1:43823/callback",
  ]);
  cmd.spotifySetClientId.mockResolvedValue(null);
  cmd.accountCancelConnect.mockResolvedValue(null);
  cmd.syncNow.mockResolvedValue(null);
  cmd.playlistsList.mockResolvedValue([]);
  cmd.spotifyCapabilities.mockResolvedValue({ premium: true, devices: [] });
  cmd.playerOutputs.mockResolvedValue(["Speakers (Realtek)", "Headphones (USB)"]);
  cmd.playerSetOutput.mockResolvedValue(null);
  cmd.playerTestSound.mockResolvedValue(null);
  cmd.localFoldersList.mockResolvedValue([]);
  cmd.localCopyTarget.mockResolvedValue("C:\\Users\\me\\Music\\Unison");
});

async function openDrawer() {
  renderRoute("/settings");
  await userEvent.click(await screen.findByRole("button", { name: a.connect }));
  return screen.findByRole("dialog", { name: a.byokTitle });
}

describe("Settings → About", () => {
  it("shows the version and schema from Rust", async () => {
    renderRoute("/settings");
    expect(await screen.findByText("0.1.0")).toBeInTheDocument();
    expect(screen.getByText(strings.settings.schema)).toBeInTheDocument();
  });
});

describe("Settings → Accounts", () => {
  it("lists providers with their status and connected name", async () => {
    cmd.accountsList.mockResolvedValue([
      { provider: "spotify", status: "connected", displayName: "Demo Listener", lastSyncedAt: null },
      notConnected[1],
    ]);
    renderRoute("/settings");
    expect(await screen.findByText("· Demo Listener")).toBeInTheDocument();
    expect(screen.getByText(a.statusConnected)).toBeInTheDocument();
    expect(screen.getByText(a.youtubeLater)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: a.reconnect })).toBeInTheDocument();
  });

  it("opens the BYOK drawer with the three redirect URIs", async () => {
    const dialog = await openDrawer();
    for (const port of [43821, 43822, 43823]) {
      expect(await within(dialog).findByText(`http://127.0.0.1:${port}/callback`)).toBeInTheDocument();
    }
    await userEvent.click(within(dialog).getByRole("button", { name: new RegExp(a.openDashboard) }));
    expect(cmd.spotifyOpenDashboard).toHaveBeenCalledOnce();
  });

  it("rejects a malformed Client ID without calling Rust", async () => {
    const dialog = await openDrawer();
    await userEvent.type(within(dialog).getByLabelText(new RegExp(a.byokStep3)), "not-an-id");
    await userEvent.click(within(dialog).getByRole("button", { name: a.connect }));
    expect(within(dialog).getByRole("alert")).toHaveTextContent(strings.errors.invalidClientId);
    expect(cmd.spotifySetClientId).not.toHaveBeenCalled();
    expect(cmd.accountConnect).not.toHaveBeenCalled();
  });

  it("asks for a Client ID when none is saved", async () => {
    const dialog = await openDrawer();
    await userEvent.click(within(dialog).getByRole("button", { name: a.connect }));
    expect(within(dialog).getByRole("alert")).toHaveTextContent(a.clientIdRequired);
  });

  it("saves the ID, connects, toasts and closes", async () => {
    cmd.accountConnect.mockResolvedValue({
      provider: "spotify",
      status: "connected",
      displayName: "Demo Listener",
      lastSyncedAt: null,
    });
    const dialog = await openDrawer();
    await userEvent.type(within(dialog).getByLabelText(new RegExp(a.byokStep3)), GOOD_ID);
    await userEvent.click(within(dialog).getByRole("button", { name: a.connect }));
    await waitFor(() => expect(cmd.accountConnect).toHaveBeenCalledWith("spotify"));
    expect(cmd.spotifySetClientId).toHaveBeenCalledWith(GOOD_ID);
    expect(await screen.findByText(a.connectedToast)).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("shows the waiting state with Cancel while the browser is open", async () => {
    cmd.spotifyHasClientId.mockResolvedValue(true);
    cmd.accountConnect.mockReturnValue(new Promise(() => undefined));
    const dialog = await openDrawer();
    expect(await within(dialog).findByText(a.clientIdSaved)).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: a.connect }));
    expect(await within(dialog).findByText(a.waiting)).toBeInTheDocument();
    expect(cmd.spotifySetClientId).not.toHaveBeenCalled();
    await userEvent.click(within(dialog).getByRole("button", { name: a.cancel }));
    expect(cmd.accountCancelConnect).toHaveBeenCalledOnce();
  });

  it("explains how to fix an account missing from User Management", async () => {
    cmd.spotifyHasClientId.mockResolvedValue(true);
    cmd.accountConnect.mockRejectedValue({
      code: { type: "userNotRegistered" },
      message: "403",
      retryable: false,
    });
    const dialog = await openDrawer();
    await within(dialog).findByText(a.clientIdSaved);
    await userEvent.click(within(dialog).getByRole("button", { name: a.connect }));
    expect(await within(dialog).findByText(strings.errors.userNotRegistered)).toBeInTheDocument();
  });

  it("syncs on demand and shows when it last synced", async () => {
    cmd.accountsList.mockResolvedValue([
      { provider: "spotify", status: "connected", displayName: "Demo", lastSyncedAt: Math.floor(Date.now() / 1000) - 300 },
      notConnected[1],
    ]);
    renderRoute("/settings");
    expect(await screen.findByText(/Synced 5 minutes ago/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: strings.sync.syncNow }));
    expect(cmd.syncNow).toHaveBeenCalledWith("spotify");
  });

  it("disables Sync now while syncing", async () => {
    cmd.accountsList.mockResolvedValue([
      { provider: "spotify", status: "syncing", displayName: "Demo", lastSyncedAt: null },
      notConnected[1],
    ]);
    renderRoute("/settings");
    expect(await screen.findByRole("button", { name: strings.sync.syncNow })).toBeDisabled();
    expect(screen.getByText(a.statusSyncing)).toBeInTheDocument();
  });
});

describe("Settings → YouTube player test", () => {
  type Events = { onStateChange?: (e: { data: number }) => void; onError?: (e: { data: number }) => void };
  let ytEvents: Events = {};
  let attachedTo: HTMLIFrameElement | null = null;

  beforeEach(() => {
    youtubeListeners.length = 0;
    window.YT = {
      Player: vi.fn(function (el: HTMLIFrameElement, opts: { events?: Events }) {
        attachedTo = el;
        ytEvents = opts.events ?? {};
      }) as unknown as NonNullable<typeof window.YT>["Player"],
    };
  });
  afterEach(() => {
    delete window.YT;
  });

  it("shows the official player with the fixed embed and reports PLAYING to Rust", async () => {
    let resolveTest: () => void = () => undefined;
    cmd.youtubeTestPlayback.mockReturnValue(new Promise<void>((r) => (resolveTest = r)));
    cmd.youtubeTestReport.mockResolvedValue(null);
    renderRoute("/settings");
    await userEvent.click(await screen.findByRole("button", { name: strings.settings.youtube.test }));
    expect(screen.getByRole("button", { name: strings.settings.youtube.testing })).toBeDisabled();

    // Rust asks the UI to play the test video.
    await waitFor(() => expect(youtubeListeners.length).toBeGreaterThan(0));
    act(() => {
      youtubeListeners.forEach((cb) => cb({ payload: { type: "test", videoId: "jNQXAC9IVRw" } }));
    });
    const frame = (await screen.findByTitle(strings.settings.youtube.title)) as HTMLIFrameElement;
    const src = new URL(frame.src);
    expect(src.origin).toBe("https://www.youtube-nocookie.com");
    expect(src.searchParams.get("origin")).toBe(window.location.origin);
    expect(frame.getAttribute("referrerpolicy")).toBe("strict-origin-when-cross-origin");
    expect(frame.className).toContain("min-h-[200px]");
    await waitFor(() => expect(attachedTo).toBe(frame));

    ytEvents.onStateChange?.({ data: 1 });
    expect(cmd.youtubeTestReport).toHaveBeenCalledWith(true, null);
    resolveTest();
    expect(await screen.findByText(strings.settings.youtube.ok)).toBeInTheDocument();
  });

  it("reports embed errors and shows the reason", async () => {
    cmd.youtubeTestPlayback.mockRejectedValue({ code: { type: "youtubeEmbedBlocked", code: 153 }, message: "x", retryable: false });
    renderRoute("/settings");
    await userEvent.click(await screen.findByRole("button", { name: strings.settings.youtube.test }));
    expect(await screen.findByText(/error 153/)).toBeInTheDocument();
  });
});

describe("Settings → Playback → Spotify", () => {
  it("shows Ready with the Spotify devices when Premium and a device is open", async () => {
    cmd.accountsList.mockResolvedValue([
      { provider: "spotify", status: "connected", displayName: "Demo", lastSyncedAt: null },
      notConnected[1],
    ]);
    cmd.spotifyCapabilities.mockResolvedValue({
      premium: true,
      devices: [{ id: "d1", name: "ASUS laptop", kind: "computer", isActive: true, isRestricted: false, volumePercent: 60, supportsVolume: true }],
    });
    renderRoute("/settings");
    expect(await screen.findByText(strings.settings.spotifyPlayback.ready)).toBeInTheDocument();
    expect(screen.getByRole("list", { name: strings.settings.spotifyPlayback.devices })).toHaveTextContent("ASUS laptop");
    await userEvent.click(screen.getByRole("button", { name: strings.settings.spotifyPlayback.refresh }));
    await waitFor(() => expect(cmd.spotifyCapabilities).toHaveBeenCalledWith(true));
  });

  it("does not ask Spotify when not connected", async () => {
    renderRoute("/settings");
    expect(await screen.findAllByText(strings.settings.spotifyPlayback.notConnected)).not.toHaveLength(0);
    expect(cmd.spotifyCapabilities).not.toHaveBeenCalled();
  });
});

describe("Settings → Playback → Output", () => {
  it("lists outputs, switches device and plays a test sound", async () => {
    const settings = { ...defaultsForTest(), player: { ...defaultsForTest().player, outputDevice: "Headphones (USB)" } };
    cmd.settingsGet.mockResolvedValue(settings);
    renderRoute("/settings");
    const select = await screen.findByRole("combobox", { name: strings.settings.output.title });
    await userEvent.click(select);
    await userEvent.click(await screen.findByRole("option", { name: "Headphones (USB)" }));
    await waitFor(() => expect(cmd.playerSetOutput).toHaveBeenCalledWith("Headphones (USB)"));
    await waitFor(() => expect(cmd.settingsGet).toHaveBeenCalled());
    await userEvent.click(screen.getByRole("button", { name: strings.settings.output.test }));
    expect(cmd.playerTestSound).toHaveBeenCalledOnce();
  });

  it("shows output errors", async () => {
    cmd.playerTestSound.mockRejectedValue({ code: { type: "outputDeviceUnavailable" }, message: "x", retryable: false });
    renderRoute("/settings");
    await userEvent.click(await screen.findByRole("button", { name: strings.settings.output.test }));
    expect(await screen.findByText(strings.errors.outputDeviceUnavailable)).toBeInTheDocument();
  });
});

describe("Settings → Local files", () => {
  it("turns copy-on-drop on and shows where copies go", async () => {
    const defaults = defaultsForTest();
    useUiStore.getState().hydrate(defaults);
    cmd.localCopyTarget.mockResolvedValue("C:\\Users\\me\\Music\\Unison");
    cmd.settingsSet.mockImplementation((next: typeof defaults) => Promise.resolve(next));
    renderRoute("/settings");
    const toggle = await screen.findByRole("switch", { name: strings.settings.local.copyOnDrop });
    expect(toggle).toHaveAttribute("aria-checked", "false");
    expect(screen.getByText(strings.settings.local.referenceHint)).toBeInTheDocument();
    await userEvent.click(toggle);
    await waitFor(() =>
      expect(cmd.settingsSet).toHaveBeenCalledWith(expect.objectContaining({ local: expect.objectContaining({ copyOnDrop: true }) })),
    );
    await waitFor(() => expect(toggle).toHaveAttribute("aria-checked", "true"));
    expect(screen.getByText(/Copies go to C:\\Users\\me\\Music\\Unison/)).toBeInTheDocument();
  });
});

describe("Settings → Local files → Watched folders", () => {
  const l = strings.settings.local;
  const music = { path: "C:\\Users\\me\\Music", lastScanAt: 1_700_000_000, exists: true };
  const usb = { path: "E:\\Music", lastScanAt: null, exists: false };

  it("lists folders with scan state and adds one through the folder picker", async () => {
    cmd.localFoldersList.mockResolvedValue([music, usb]);
    cmd.localPickFolder.mockResolvedValue("D:\\Albums");
    const added = { path: "D:\\Albums", lastScanAt: null, exists: true };
    cmd.localFolderAdd.mockResolvedValue([music, usb, added]);
    renderRoute("/settings");
    const list = await screen.findByRole("list", { name: l.foldersTitle });
    expect(within(list).getByText(music.path)).toBeInTheDocument();
    expect(within(list).getByText(/^Scanned /)).toBeInTheDocument();
    expect(within(list).getByText(l.folderMissing)).toBeInTheDocument();
    expect(screen.getByText(/^Last scan /)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: l.addFolder }));
    expect(cmd.localPickFolder).toHaveBeenCalledWith(l.folderDialogTitle);
    await waitFor(() => expect(cmd.localFolderAdd).toHaveBeenCalledWith("D:\\Albums"));
    expect(await within(list).findByText("D:\\Albums")).toBeInTheDocument();
    expect(within(list).getByText(l.notScanned)).toBeInTheDocument();
  });

  it("removes a folder, does nothing when the picker is cancelled, and rescans", async () => {
    cmd.localFoldersList.mockResolvedValue([music]);
    cmd.localFolderRemove.mockResolvedValue([]);
    cmd.localPickFolder.mockResolvedValue(null);
    cmd.localRescan.mockResolvedValue(7);
    renderRoute("/settings");
    await screen.findByText(music.path);
    await userEvent.click(screen.getByRole("button", { name: l.addFolder }));
    expect(cmd.localFolderAdd).not.toHaveBeenCalled();

    await userEvent.click(screen.getByRole("button", { name: l.removeFolder.replace("{path}", music.path) }));
    expect(cmd.localFolderRemove).toHaveBeenCalledWith(music.path);
    expect(await screen.findByText(l.noFolders)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: l.rescan }));
    expect(cmd.localRescan).toHaveBeenCalledOnce();
  });

  it("shows an error when adding fails", async () => {
    cmd.localPickFolder.mockResolvedValue("C:\\gone");
    cmd.localFolderAdd.mockRejectedValue({ code: { type: "fileMissing" }, message: "folder not found", retryable: false });
    renderRoute("/settings");
    expect(await screen.findByText(l.noFolders)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: l.addFolder }));
    expect(await screen.findByRole("alert")).toHaveTextContent(strings.errors.fileMissing);
  });
});

function defaultsForTest() {
  return {
    theme: "dark" as const, glassFrost: 0.5, windowMaterial: "mica" as const, sidebarCollapsed: [], syncIntervalMin: 30, preferredSource: null,
    player: { sourcePriority: ["local", "spotify", "youtube", "audius"] as Array<"local" | "spotify" | "youtube" | "audius">, autoFallback: true, fallbackOnErrorMidtrack: true,
      spotifyMode: "auto" as const, youtubeRelayUrl: null, outputDevice: null as string | null, gapless: true, crossfadeMs: 0, replaygain: "track" as const, volume: 80 },
    local: { watchedFolders: [], copyOnDrop: false, copyTarget: null, extensions: ["mp3"] },
    audius: { enabled: true, useAsFallback: true, showInSearch: true, showTrendingRow: false },
  };
}
