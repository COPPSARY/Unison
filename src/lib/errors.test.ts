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
    expect(messageForCode({ type: "youtubeEmbedBlocked", code: 153 })).toContain("153");
    expect(messageForCode({ type: "youtubeQuotaExceeded", resetsAt: 0 })).not.toContain("{time}");
  });

  it("has a message for every code", () => {
    const codes: ErrorCode[] = [
      { type: "offline" },
      { type: "authCancelled" },
      { type: "authTimeout" },
      { type: "needsReconnect" },
      { type: "invalidClientId" },
      { type: "redirectUriMismatch" },
      { type: "spotifyPremiumRequired" },
      { type: "noSpotifyDevice" },
      { type: "rateLimited", retryAfter: 1 },
      { type: "youtubeQuotaExceeded", resetsAt: 1 },
      { type: "notEmbeddable" },
      { type: "notFound" },
      { type: "upstream", status: 500 },
      { type: "internal" },
      { type: "userNotRegistered" },
      { type: "noPlayableSource" },
      { type: "fileMissing" },
      { type: "unsupportedFormat" },
      { type: "drmProtected" },
      { type: "audiusUnavailable" },
      { type: "youtubeEmbedBlocked", code: 153 },
      { type: "outputDeviceUnavailable" },
    ];
    for (const code of codes) expect(messageForCode(code).length).toBeGreaterThan(10);
  });
});
