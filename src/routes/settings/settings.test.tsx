import { screen } from "@testing-library/react";
import { strings } from "@/i18n";
import { renderRoute } from "@/test/render";

const appInfo = vi.fn();

vi.mock("@/lib/bindings", () => ({ commands: { appInfo: () => appInfo() } }));
vi.mock("@/lib/window", () => ({
  isTauri: () => true,
  windowControls: {
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
    onMaximizedChange: () => () => undefined,
  },
}));

describe("Settings → About", () => {
  it("shows the version and schema from Rust", async () => {
    appInfo.mockResolvedValue({ version: "0.1.0", schemaVersion: 1 });
    renderRoute("/settings");
    expect(await screen.findByText("0.1.0")).toBeInTheDocument();
    expect(screen.getByText(strings.settings.schema)).toBeInTheDocument();
  });

  it("shows the mapped error message when the command fails", async () => {
    appInfo.mockRejectedValue({ code: { type: "internal" }, message: "db", retryable: false });
    renderRoute("/settings");
    expect(await screen.findByRole("alert")).toHaveTextContent(strings.errors.internal);
  });
});
