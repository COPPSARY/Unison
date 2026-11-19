import { useUiStore } from "./ui";

describe("ui store", () => {
  it("defaults to dark and writes the theme to <html>", () => {
    expect(useUiStore.getState().theme).toBe("dark");
    expect(document.documentElement.dataset["theme"]).toBe("dark");
    useUiStore.getState().setTheme("light");
    expect(document.documentElement.dataset["theme"]).toBe("light");
  });

  it("re-derives the accent for the theme and clamps frost", () => {
    const style = document.documentElement.style;
    useUiStore.getState().setAccentSource({ r: 196, g: 32, b: 54 });
    const light = style.getPropertyValue("--accent");
    useUiStore.getState().setTheme("dark");
    expect(style.getPropertyValue("--accent")).not.toBe(light);
    useUiStore.getState().setGlassFrost(3);
    expect(style.getPropertyValue("--glass-frost")).toBe("1");
  });
});
