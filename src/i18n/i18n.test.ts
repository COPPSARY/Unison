import { format } from ".";

describe("format", () => {
  it("fills placeholders and leaves unknown ones", () => {
    expect(format("Added to {name}", { name: "Mix" })).toBe("Added to Mix");
    expect(format("{count} new, {x}", { count: 3 })).toBe("3 new, {x}");
  });
});
