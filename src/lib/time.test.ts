import { strings } from "@/i18n";
import { relativeTime } from "./time";

describe("relativeTime", () => {
  const now = 1_700_000_000_000;
  it("says just now under a minute", () => {
    expect(relativeTime(now / 1000 - 20, now)).toBe(strings.common.justNow);
  });
  it("uses the largest whole unit", () => {
    expect(relativeTime(now / 1000 - 5 * 60, now)).toMatch(/5 minutes ago/);
    expect(relativeTime(now / 1000 - 3 * 3600, now)).toMatch(/3 hours ago/);
    expect(relativeTime(now / 1000 - 26 * 3600, now)).toMatch(/yesterday|1 day ago/);
  });
});
