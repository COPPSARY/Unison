import { strings } from "@/i18n";
import type { ErrorCode } from "./bindings";
import { errorMessage, isAppError, messageForCode } from "./errors";

describe("errors", () => {
  it("recognises AppError rejections", () => {
    const err = { code: { type: "offline" }, message: "x", retryable: true };
    expect(isAppError(err)).toBe(true);
    expect(isAppError("boom")).toBe(false);
    expect(isAppError({ message: "x" })).toBe(false);
    expect(errorMessage(err)).toBe(strings.errors.offline);
    expect(errorMessage(new Error("framework"))).toBe(strings.errors.unknown);
  });

  it("fills data-carrying codes", () => {
    expect(messageForCode({ type: "rateLimited", retryAfter: 30 })).toContain("30 s");
    expect(messageForCode({ type: "upstream", status: 503 })).toContain("503");
  });

  it("has a message for every code", () => {
    const codes: ErrorCode[] = [
      { type: "offline" },
      { type: "rateLimited", retryAfter: 1 },
      { type: "notFound" },
      { type: "upstream", status: 500 },
      { type: "internal" },
      { type: "noPlayableSource" },
      { type: "fileMissing" },
      { type: "unsupportedFormat" },
      { type: "drmProtected" },
      { type: "audiusUnavailable" },
      { type: "outputDeviceUnavailable" },
      { type: "extensionLinkInvalid" },
      { type: "extensionInvalid" },
      { type: "extensionHostNotAllowed" },
      { type: "extensionDisabled" },
      { type: "extensionFailed" },
      { type: "extensionRegistryInvalid" },
      { type: "extensionUnsupported" },
      { type: "extensionUnsupportedFeature", extension: "amazon", feature: "signedSession@1" },
    ];
    for (const code of codes) expect(messageForCode(code).length).toBeGreaterThan(10);
  });
});
