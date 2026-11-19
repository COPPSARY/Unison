/**
 * YouTube IFrame Player API (official). Loaded once from www.youtube.com (allowed by the CSP
 * script-src); the player itself runs on www.youtube-nocookie.com (frame-src). No audio is
 * extracted; the player must stay visible at >= 200x200 px.
 */

export type YTPlayer = {
  playVideo: () => void;
  pauseVideo: () => void;
  stopVideo: () => void;
  seekTo: (seconds: number, allowSeekAhead: boolean) => void;
  loadVideoById: (videoId: string, startSeconds?: number) => void;
  setVolume: (volume: number) => void;
  getCurrentTime: () => number;
  getDuration: () => number;
  destroy: () => void;
};

type YTEvent<T = unknown> = { data: T; target: YTPlayer };

export type YTPlayerOptions = {
  events?: {
    onReady?: (e: YTEvent) => void;
    onStateChange?: (e: YTEvent<number>) => void;
    onError?: (e: YTEvent<number>) => void;
  };
};

type YTNamespace = { Player: new (el: HTMLIFrameElement, opts: YTPlayerOptions) => YTPlayer };

declare global {
  interface Window {
    YT?: YTNamespace;
    onYouTubeIframeAPIReady?: () => void;
  }
}

/** Player states from the IFrame API. */
export const YT_STATE = { unstarted: -1, ended: 0, playing: 1, paused: 2, buffering: 3, cued: 5 } as const;

export const IFRAME_API_URL = "https://www.youtube.com/iframe_api";
export const EMBED_HOST = "https://www.youtube-nocookie.com";

let apiPromise: Promise<YTNamespace> | null = null;

/** Loads the IFrame API once. */
export function loadIframeApi(): Promise<YTNamespace> {
  if (window.YT?.Player) return Promise.resolve(window.YT);
  apiPromise ??= new Promise<YTNamespace>((resolve, reject) => {
    const previous = window.onYouTubeIframeAPIReady;
    window.onYouTubeIframeAPIReady = () => {
      previous?.();
      if (window.YT?.Player) resolve(window.YT);
      else reject(new Error("YouTube IFrame API loaded without YT.Player"));
    };
    const script = document.createElement("script");
    script.src = IFRAME_API_URL;
    script.async = true;
    script.onerror = () => {
      apiPromise = null;
      reject(new Error("YouTube IFrame API failed to load"));
    };
    document.head.appendChild(script);
  });
  return apiPromise;
}

/**
 * Embed URL per the addendum's fix: nocookie host, JS API on, explicit origin.
 * `origin` must be the app's own origin (https://tauri.localhost in production).
 */
export function embedUrl(videoId: string, origin: string, autoplay = true): string {
  const params = new URLSearchParams({
    enablejsapi: "1",
    origin,
    playsinline: "1",
    rel: "0",
    autoplay: autoplay ? "1" : "0",
  });
  return `${EMBED_HOST}/embed/${encodeURIComponent(videoId)}?${params.toString()}`;
}

/** Relay page URL (owner-hosted, see docs/youtube-relay/yt.html). */
export function relayUrl(base: string, videoId: string, origin: string): string {
  const url = new URL(base);
  url.searchParams.set("v", videoId);
  url.searchParams.set("origin", origin);
  return url.toString();
}

/** Messages exchanged with the relay page. */
export type RelayMessage =
  | { unison: "yt"; event: "ready" }
  | { unison: "yt"; event: "state"; info: number }
  | { unison: "yt"; event: "error"; info: number };

export function isRelayMessage(data: unknown): data is RelayMessage {
  return typeof data === "object" && data !== null && (data as { unison?: unknown }).unison === "yt";
}
