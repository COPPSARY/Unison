import {
  SPRINGS,
  installSprings,
  settleTime,
  springEasing,
  springPosition,
  type SpringParams,
} from "./spring";

const peak = (p: SpringParams) => {
  let max = 0;
  for (let i = 0; i < 2000; i++) max = Math.max(max, springPosition(p, i / 1000));
  return max;
};

describe("springs", () => {
  it("starts at 0 and settles at 1", () => {
    for (const p of Object.values(SPRINGS)) {
      expect(springPosition(p, 0)).toBeCloseTo(0, 6);
      expect(springPosition(p, settleTime(p))).toBeCloseTo(1, 2);
    }
  });

  it("smooth never overshoots; snappy overshoots slightly", () => {
    expect(peak(SPRINGS.smooth)).toBeLessThanOrEqual(1);
    const snappyPeak = peak(SPRINGS.snappy);
    expect(snappyPeak).toBeGreaterThan(1.005);
    expect(snappyPeak).toBeLessThan(1.1);
  });

  it("produces a CSS linear() easing that ends at 1", () => {
    const { easing, durationMs } = springEasing(SPRINGS.smooth);
    expect(easing.startsWith("linear(0, ")).toBe(true);
    expect(easing.endsWith(", 1)")).toBe(true);
    expect(durationMs).toBeGreaterThan(300);
    expect(durationMs).toBeLessThan(900);
  });

  it("installs CSS variables", () => {
    installSprings();
    const style = document.documentElement.style;
    expect(style.getPropertyValue("--spring-smooth")).toMatch(/^linear\(/);
    expect(style.getPropertyValue("--spring-snappy-duration")).toMatch(/^\d+ms$/);
  });
});
