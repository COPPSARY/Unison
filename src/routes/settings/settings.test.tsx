import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { strings } from "@/i18n";
import { renderRoute } from "@/test/render";
import { useUiStore } from "@/stores/ui";

const cmd = {
  appInfo: vi.fn(),
  playlistsList: vi.fn(),
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
    localDrag: { listen: () => Promise.resolve(() => undefined) },
    localImportProgress: { listen: () => Promise.resolve(() => undefined) },
    localChanged: { listen: () => Promise.resolve(() => undefined) },
    downloadProgress: { listen: () => Promise.resolve(() => undefined) },
    playerState: { listen: () => Promise.resolve(() => undefined) },
    playerFallback: { listen: () => Promise.resolve(() => undefined) },
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

beforeEach(() => {
  for (const fn of Object.values(cmd)) fn.mockReset();
  cmd.appInfo.mockResolvedValue({ version: "0.1.0", schemaVersion: 1 });
  cmd.playlistsList.mockResolvedValue([]);
  cmd.playerOutputs.mockResolvedValue(["Speakers (Realtek)", "Headphones (USB)"]);
  cmd.playerSetOutput.mockResolvedValue(null);
  cmd.playerTestSound.mockResolvedValue(null);
  cmd.localFoldersList.mockResolvedValue([]);
  cmd.localCopyTarget.mockResolvedValue("C:\\Users\\me\\Music\\Unison");
});

describe("Settings → About", () => {
  it("shows the version and schema from Rust, and no accounts or extensions section", async () => {
    renderRoute("/settings");
    expect(await screen.findByText("0.1.0")).toBeInTheDocument();
    expect(screen.getByText(strings.settings.schema)).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: strings.extensions.title })).not.toBeInTheDocument();
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

describe("Settings → Playback → Stream quality", () => {
  it("defaults to High and saves another tier", async () => {
    cmd.settingsGet.mockResolvedValue(defaultsForTest());
    cmd.settingsSet.mockImplementation((v: unknown) => Promise.resolve(v));
    renderRoute("/settings");
    const select = await screen.findByRole("combobox", { name: strings.settings.streamQuality.title });
    expect(select).toHaveTextContent(strings.settings.streamQuality.high);
    await userEvent.click(select);
    await userEvent.click(await screen.findByRole("option", { name: strings.settings.streamQuality.lossless }));
    await waitFor(() =>
      expect(cmd.settingsSet).toHaveBeenCalledWith(
        expect.objectContaining({ player: expect.objectContaining({ streamQuality: "lossless" }) }),
      ),
    );
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
    theme: "dark" as const, glassFrost: 0.5, windowMaterial: "mica" as const, sidebarCollapsed: [],
    player: { sourcePriority: ["local", "audius", "extension"] as Array<"local" | "audius" | "extension">, autoFallback: true, fallbackOnErrorMidtrack: true, streamQuality: "high" as "best" | "lossless" | "high" | "low",
      outputDevice: null as string | null, gapless: true, crossfadeMs: 0, replaygain: "track" as const, volume: 80 },
    local: { watchedFolders: [], copyOnDrop: false, copyTarget: null, extensions: ["mp3"] },
    audius: { enabled: true, useAsFallback: true, showInSearch: true, showTrendingRow: false },
  };
}
