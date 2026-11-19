import { embedUrl, isRelayMessage, relayUrl } from "./youtube";

describe("youtube embed URLs", () => {
  it("uses the nocookie host with the JS API and the app origin", () => {
    const url = new URL(embedUrl("abc_123", "https://tauri.localhost"));
    expect(url.origin).toBe("https://www.youtube-nocookie.com");
    expect(url.pathname).toBe("/embed/abc_123");
    expect(url.searchParams.get("enablejsapi")).toBe("1");
    expect(url.searchParams.get("origin")).toBe("https://tauri.localhost");
  });

  it("builds relay URLs and recognises relay messages", () => {
    const url = new URL(relayUrl("https://player.example.com/yt.html", "v1", "https://tauri.localhost"));
    expect(url.searchParams.get("v")).toBe("v1");
    expect(isRelayMessage({ unison: "yt", event: "ready" })).toBe(true);
    expect(isRelayMessage({ event: "ready" })).toBe(false);
  });
});
