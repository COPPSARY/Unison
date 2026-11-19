import { strings } from "@/i18n";
import { spotifyStatus } from "./spotifyStatus";

const t = strings.settings.spotifyPlayback;
const dev = (over: Record<string, unknown> = {}) => ({
  id: "d1", name: "PC", kind: "computer", isActive: true, isRestricted: false, volumePercent: 50, supportsVolume: true, ...over,
});

describe("spotifyStatus", () => {
  it("covers every state", () => {
    expect(spotifyStatus(undefined, false).label).toBe(t.notConnected);
    expect(spotifyStatus({ premium: false, devices: [dev()] }, true).label).toBe(t.needsPremium);
    expect(spotifyStatus({ premium: null, devices: [] }, true).label).toBe(t.noDevice);
    expect(spotifyStatus({ premium: true, devices: [dev({ isRestricted: true })] }, true).label).toBe(t.noDevice);
    expect(spotifyStatus({ premium: null, devices: [dev()] }, true).label).toBe(t.unknown);
    expect(spotifyStatus({ premium: true, devices: [dev()] }, true).label).toBe(t.ready);
  });
});
