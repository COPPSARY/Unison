import { applyAccent, contrast, deriveAccent, type Rgb } from "./accent";

const DARK_BG: Rgb = { r: 18, g: 18, b: 22 };
const LIGHT_BG: Rgb = { r: 244, g: 244, b: 246 };

const samples: Rgb[] = [
  { r: 200, g: 30, b: 40 }, // crimson cover
  { r: 10, g: 60, b: 70 }, // very dark teal
  { r: 250, g: 230, b: 120 }, // pale yellow
  { r: 30, g: 90, b: 220 }, // blue
];

describe("deriveAccent", () => {
  it.each(samples)("meets 3:1 against both backgrounds for %o", (src) => {
    expect(contrast(deriveAccent(src, "dark").accent, DARK_BG)).toBeGreaterThanOrEqual(3);
    expect(contrast(deriveAccent(src, "light").accent, LIGHT_BG)).toBeGreaterThanOrEqual(3);
  });

  it("picks a readable foreground", () => {
    for (const src of samples) {
      for (const theme of ["dark", "light"] as const) {
        const { accent, foreground } = deriveAccent(src, theme);
        expect(contrast(accent, foreground)).toBeGreaterThanOrEqual(3);
      }
    }
  });

  it("falls back to violet for greyscale artwork or no artwork", () => {
    const grey = deriveAccent({ r: 128, g: 128, b: 128 }, "dark").accent;
    const none = deriveAccent(null, "dark").accent;
    expect(grey).toEqual(none);
    expect(none.b).toBeGreaterThan(none.r);
    expect(none.g).toBeLessThan(none.r);
  });

  it("keeps the artwork hue", () => {
    const { accent } = deriveAccent({ r: 200, g: 30, b: 40 }, "dark");
    expect(accent.r).toBeGreaterThan(accent.g);
    expect(accent.r).toBeGreaterThan(accent.b);
  });

  it("writes CSS variables", () => {
    applyAccent({ r: 30, g: 90, b: 220 }, "dark");
    const style = document.documentElement.style;
    expect(style.getPropertyValue("--accent")).toMatch(/^rgb\(\d+ \d+ \d+\)$/);
    expect(style.getPropertyValue("--accent-rgb")).toMatch(/^\d+ \d+ \d+$/);
  });
});
