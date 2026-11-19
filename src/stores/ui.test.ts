import { useUiStore } from "./ui";

describe("ui store", () => {
  it("defaults to dark and writes the theme to <html>", () => {
    expect(useUiStore.getState().theme).toBe("dark");
    expect(document.documentElement.dataset["theme"]).toBe("dark");
    useUiStore.getState().setTheme("light");
    expect(document.documentElement.dataset["theme"]).toBe("light");
  });
});
