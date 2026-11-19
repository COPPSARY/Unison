import { screen } from "@testing-library/react";
import { strings } from "@/i18n";
import { renderRoute } from "@/test/render";

describe("routes", () => {
  it("renders the wordmark at /", () => {
    renderRoute("/");
    expect(screen.getByRole("heading", { name: strings.app.name })).toBeInTheDocument();
  });

  it("renders every UI kit section at /dev/ui", () => {
    renderRoute("/dev/ui");
    const s = strings.devUi;
    expect(screen.getByRole("heading", { level: 1, name: s.title })).toBeInTheDocument();
    for (const title of [
      s.materials, s.colors, s.typography, s.buttons, s.iconButtons, s.inputs, s.selects, s.segmented,
      s.menu, s.dialog, s.drawer, s.tooltip, s.toast, s.skeleton, s.banner,
    ]) {
      expect(screen.getByRole("heading", { level: 2, name: title })).toBeInTheDocument();
    }
  });
});
