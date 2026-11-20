import { screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { format, strings } from "@/i18n";
import { renderRoute } from "@/test/render";

const cmd = {
  playlistsList: vi.fn(),
  extensionList: vi.fn(),
  extensionRegistries: vi.fn(),
  extensionRegistry: vi.fn(),
  extensionRegistryAdd: vi.fn(),
  extensionRegistryRemove: vi.fn(),
  extensionPreview: vi.fn(),
  extensionInstall: vi.fn(),
  extensionSetEnabled: vi.fn(),
  extensionRemove: vi.fn(),
  extensionSearch: vi.fn(),
  extensionSaveTracks: vi.fn(),
  playerPlay: vi.fn(),
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
  windowControls: {
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
    onMaximizedChange: () => () => undefined,
  },
}));

const t = strings.extensions;
const SHA = "0123456789abcdef0123456789abcdef01234567";
const PKG = "https://example.com/ext/mock-stream.spotiflac-ext";
const PKG_SHA = "ab".repeat(32);
const REGISTRY = "https://raw.githubusercontent.com/acme/hub/main/registry.json";
const ICON = "data:image/png;base64,iVBORw0KGgo=";
const NO_LISTING = { id: null, name: null, version: null, description: null, category: null, tags: [] };

type Cap = "search" | "play";

const preview = (over: Record<string, unknown> = {}) => ({
  id: "mock-stream",
  name: "Mock Stream",
  version: "2.0.0",
  author: "Acme",
  description: "Streams free music.\nSecond line.",
  hosts: ["api.mock.example", "*.cdn.mock.example"],
  repoUrl: PKG,
  commitSha: PKG_SHA,
  installedVersion: null,
  runsCode: true,
  capabilities: ["search", "play"] as Cap[],
  types: ["metadata_provider", "download_provider"],
  homepage: "https://mock.example",
  icon: ICON,
  ...over,
});

const installed = (over: Record<string, unknown> = {}) => ({
  id: "mock-stream",
  name: "Mock Stream",
  version: "2.0.0",
  author: "Acme",
  description: "Streams free music.",
  hosts: ["api.mock.example"],
  repoUrl: PKG,
  commitSha: PKG_SHA,
  enabled: true,
  installedAt: 1_763_500_000,
  runsCode: true,
  capabilities: ["search", "play"] as Cap[],
  types: ["metadata_provider"],
  homepage: null,
  icon: ICON,
  ...over,
});

const registry = (items: unknown[]) => ({ url: REGISTRY, commitSha: SHA, items });
const listedItem = (p: ReturnType<typeof preview>) => ({
  entry: p.repoUrl,
  listed: { ...NO_LISTING, id: p.id, tags: ["free", "radio"], category: "streaming" },
  preview: p,
  error: null,
});

beforeEach(() => {
  for (const fn of Object.values(cmd)) fn.mockReset();
  cmd.playlistsList.mockResolvedValue([]);
  cmd.extensionList.mockResolvedValue([]);
  cmd.extensionRegistries.mockResolvedValue([]);
});

describe("Extensions page", () => {
  it("starts empty and is reachable from the sidebar", async () => {
    renderRoute("/");
    await userEvent.click(within(screen.getByRole("navigation", { name: strings.nav.sidebar })).getByRole("link", { name: strings.nav.extensions }));
    expect(await screen.findByRole("heading", { level: 1, name: t.title })).toBeInTheDocument();
    expect(await screen.findByText(t.emptyTitle)).toBeInTheDocument();
  });

  it("adds a registry and lists its extensions with icon, name, version and Install", async () => {
    cmd.extensionRegistryAdd.mockResolvedValue(
      registry([
        listedItem(preview()),
        {
          entry: "https://example.com/not-an-extension",
          listed: { ...NO_LISTING, name: "Broken One", version: "0.1.0" },
          preview: null,
          error: { code: { type: "extensionUnsupported" }, message: "x", retryable: false },
        },
      ]),
    );
    cmd.extensionInstall.mockResolvedValue([installed()]);
    renderRoute("/extensions");
    await screen.findByText(t.emptyTitle);

    // Saved registries come back from Rust once added.
    cmd.extensionRegistries.mockResolvedValue([REGISTRY]);
    cmd.extensionRegistry.mockImplementation(() => cmd.extensionRegistryAdd.mock.results[0]?.value);
    await userEvent.click(screen.getByLabelText(t.linkLabel));
    await userEvent.paste(REGISTRY);
    expect(cmd.extensionRegistryAdd).toHaveBeenCalledWith(REGISTRY);

    const list = await screen.findByRole("list", { name: format(t.registryTitle, { name: "acme/hub" }) });
    const row = within(list).getByRole("link", { name: format(t.openDetails, { name: "Mock Stream" }) });
    expect(row).toHaveAttribute("href", "/extensions/mock-stream");
    expect(row.querySelector("img")).toHaveAttribute("src", ICON);
    expect(within(row).getByText(/Version 2\.0\.0/)).toBeInTheDocument();
    expect(within(list).getByText(strings.errors.extensionUnsupported)).toBeInTheDocument();
    expect(within(list).getByRole("button", { name: format(t.installName, { name: "Broken One" }) })).toBeDisabled();

    await userEvent.click(within(list).getByRole("button", { name: format(t.installName, { name: "Mock Stream" }) }));
    expect(cmd.extensionInstall).toHaveBeenCalledWith(PKG, PKG_SHA);
    const mine = await screen.findByRole("list", { name: t.installedTitle });
    expect(within(mine).getByText("Mock Stream")).toBeInTheDocument();
    await waitFor(() =>
      expect(within(list).getByRole("button", { name: format(t.installName, { name: "Mock Stream" }) })).toHaveTextContent(
        t.installedAlready,
      ),
    );
  });

  it("offers Update when a registry has a newer version", async () => {
    cmd.extensionList.mockResolvedValue([installed({ version: "1.0.0" })]);
    cmd.extensionRegistries.mockResolvedValue([REGISTRY]);
    cmd.extensionRegistry.mockResolvedValue(registry([listedItem(preview())]));
    renderRoute("/extensions");
    const mine = await screen.findByRole("list", { name: t.installedTitle });
    expect(await within(mine).findByRole("button", { name: format(t.updateName, { name: "Mock Stream" }) })).toBeEnabled();
    expect(within(mine).getByText(format(t.updateFrom, { from: "1.0.0", to: "2.0.0" }), { exact: false })).toBeInTheDocument();
  });

  it("previews a single extension link with its hash and installs it", async () => {
    cmd.extensionPreview.mockResolvedValue(preview({ capabilities: [], runsCode: true }));
    cmd.extensionInstall.mockResolvedValue([installed()]);
    renderRoute("/extensions");
    await screen.findByText(t.emptyTitle);
    await userEvent.type(screen.getByLabelText(t.linkLabel), PKG);
    await userEvent.click(screen.getByRole("button", { name: t.add }));
    const dialog = await screen.findByRole("dialog", { name: format(t.previewTitle, { name: "Mock Stream" }) });
    expect(within(dialog).getByText(format(t.packageHash, { sha: PKG_SHA.slice(0, 12) }))).toBeInTheDocument();
    expect(within(dialog).getByText(t.runsCode)).toBeInTheDocument();
    expect(within(dialog).getByText(t.addsNothing)).toBeInTheDocument();
    expect(within(within(dialog).getByRole("list", { name: t.hosts })).getByText("*.cdn.mock.example")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: t.install }));
    expect(cmd.extensionInstall).toHaveBeenCalledWith(PKG, PKG_SHA);
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("shows why a link was rejected", async () => {
    cmd.extensionPreview.mockRejectedValue({ code: { type: "extensionLinkInvalid" }, message: "x", retryable: false });
    renderRoute("/extensions");
    await screen.findByText(t.emptyTitle);
    await userEvent.type(screen.getByLabelText(t.linkLabel), "https://example.com/x");
    await userEvent.click(screen.getByRole("button", { name: t.add }));
    expect(await screen.findByText(strings.errors.extensionLinkInvalid)).toBeInTheDocument();
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("removes a saved registry", async () => {
    cmd.extensionRegistries.mockResolvedValue([REGISTRY]);
    cmd.extensionRegistry.mockResolvedValue(registry([listedItem(preview())]));
    cmd.extensionRegistryRemove.mockResolvedValue([]);
    renderRoute("/extensions");
    await userEvent.click(await screen.findByRole("button", { name: format(t.removeRegistry, { name: "acme/hub" }) }));
    expect(cmd.extensionRegistryRemove).toHaveBeenCalledWith(REGISTRY);
    await waitFor(() =>
      expect(screen.queryByRole("list", { name: format(t.registryTitle, { name: "acme/hub" }) })).not.toBeInTheDocument(),
    );
  });

  it("shows a registry that can't be read with Retry", async () => {
    cmd.extensionRegistries.mockResolvedValue([REGISTRY]);
    cmd.extensionRegistry.mockRejectedValue({ code: { type: "offline" }, message: "x", retryable: false });
    renderRoute("/extensions");
    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent(strings.errors.offline);
    cmd.extensionRegistry.mockResolvedValue(registry([listedItem(preview())]));
    await userEvent.click(screen.getByRole("button", { name: strings.common.retry }));
    expect(await screen.findByRole("link", { name: format(t.openDetails, { name: "Mock Stream" }) })).toBeInTheDocument();
  });
});

describe("Extension details", () => {
  it("shows about, tags, capabilities and information, and installs", async () => {
    cmd.extensionRegistries.mockResolvedValue([REGISTRY]);
    cmd.extensionRegistry.mockResolvedValue(registry([listedItem(preview())]));
    cmd.extensionInstall.mockResolvedValue([installed()]);
    renderRoute("/extensions/mock-stream");

    expect(await screen.findByRole("heading", { level: 1, name: "Mock Stream" })).toBeInTheDocument();
    const about = screen.getByRole("region", { name: t.about });
    expect(within(about).getByText(/Streams free music\./)).toBeInTheDocument();
    const tags = within(screen.getByRole("region", { name: t.tags })).getByRole("list", { name: t.tags });
    expect(within(tags).getAllByRole("listitem").map((li) => li.textContent)).toEqual(["streaming", "free", "radio"]);

    const caps = screen.getByRole("region", { name: t.capabilities });
    expect(within(caps).getByText(t.typeMetadata)).toBeInTheDocument();
    expect(within(caps).getByText(t.typeDownload)).toBeInTheDocument();
    expect(within(caps).getByText(t.typeDownloadBody)).toBeInTheDocument();
    expect(within(within(caps).getByRole("list", { name: t.inUnison })).getByText(t.canPlay)).toBeInTheDocument();

    const info = screen.getByRole("region", { name: t.information });
    expect(within(info).getByText("https://mock.example")).toBeInTheDocument();
    expect(within(info).getByText(t.yes)).toBeInTheDocument();
    expect(within(info).getByText("*.cdn.mock.example")).toBeInTheDocument();
    expect(within(info).getByText(format(t.packageHash, { sha: PKG_SHA.slice(0, 12) }))).toBeInTheDocument();
    expect(within(info).getByText("acme/hub")).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: t.install }));
    expect(cmd.extensionInstall).toHaveBeenCalledWith(PKG, PKG_SHA);
    expect(await screen.findByRole("switch", { name: format(t.enable, { name: "Mock Stream" }) })).toBeChecked();
  });

  it("turns an installed extension off and removes it", async () => {
    cmd.extensionList.mockResolvedValue([installed()]);
    cmd.extensionSetEnabled.mockResolvedValue([installed({ enabled: false })]);
    cmd.extensionRemove.mockResolvedValue([]);
    renderRoute("/extensions/mock-stream");
    await userEvent.click(await screen.findByRole("switch", { name: format(t.enable, { name: "Mock Stream" }) }));
    expect(cmd.extensionSetEnabled).toHaveBeenCalledWith("mock-stream", false);
    expect(await screen.findByText(t.off)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: format(t.remove, { name: "Mock Stream" }) }));
    expect(cmd.extensionRemove).toHaveBeenCalledWith("mock-stream");
    expect(await screen.findByText(t.notFoundTitle)).toBeInTheDocument();
  });

  it("says when an extension doesn't exist", async () => {
    renderRoute("/extensions/nope");
    expect(await screen.findByText(t.notFoundTitle)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: t.backToList })).toHaveAttribute("href", "/extensions");
  });
});
