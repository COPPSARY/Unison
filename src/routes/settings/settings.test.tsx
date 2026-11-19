import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { strings } from "@/i18n";
import { renderRoute } from "@/test/render";

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
};

vi.mock("@/lib/bindings", () => ({
  events: {
    syncProgress: { listen: () => Promise.resolve(() => undefined) },
    syncFinished: { listen: () => Promise.resolve(() => undefined) },
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
