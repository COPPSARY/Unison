import { useEffect, useRef } from "react";
import { cn } from "@/lib/cn";
import {
  embedUrl,
  isRelayMessage,
  loadIframeApi,
  relayUrl,
  type YTPlayer,
} from "@/lib/youtube";

export type YouTubeHostProps = {
  videoId: string;
  /** Start position (ms). */
  startMs?: number;
  /** Receives the API player once it is attached (direct embed only). */
  onPlayer?: (player: YTPlayer | null) => void;
  /** Owner-hosted relay page (Settings → Advanced); null = direct embed. */
  relay?: string | null;
  onReady?: () => void;
  onStateChange?: (state: number) => void;
  onError?: (code: number) => void;
  className?: string;
  title: string;
};

/**
 * Official YouTube IFrame player (addendum §0 fix). The iframe is created here — nocookie host,
 * `origin`, `strict-origin-when-cross-origin` referrer — and the IFrame API attaches to it.
 * Never rendered smaller than 200×200 px or hidden while playing.
 */
export function YouTubeHost({
  videoId,
  startMs = 0,
  onPlayer,
  relay,
  onReady,
  onStateChange,
  onError,
  className,
  title,
}: YouTubeHostProps) {
  const frameRef = useRef<HTMLIFrameElement>(null);
  const playerRef = useRef<YTPlayer | null>(null);
  const handlers = useRef({ onReady, onStateChange, onError, onPlayer });
  handlers.current = { onReady, onStateChange, onError, onPlayer };

  const origin = window.location.origin;
  const src = relay ? relayUrl(relay, videoId, origin) : embedUrl(videoId, origin, true, startMs / 1000);

  // Direct embed: attach the official API to our iframe.
  useEffect(() => {
    if (relay) return;
    const frame = frameRef.current;
    if (!frame) return;
    let disposed = false;
    loadIframeApi()
      .then((YT) => {
        if (disposed) return;
        playerRef.current = new YT.Player(frame, {
          events: {
            onReady: (e) => {
              handlers.current.onPlayer?.(e.target);
              handlers.current.onReady?.();
            },
            onStateChange: (e) => handlers.current.onStateChange?.(e.data),
            onError: (e) => handlers.current.onError?.(e.data),
          },
        });
      })
      .catch(() => handlers.current.onError?.(0));
    return () => {
      disposed = true;
      playerRef.current = null;
      handlers.current.onPlayer?.(null);
    };
  }, [relay, src]);

  // Relay page: it hosts the API and forwards events by postMessage.
  useEffect(() => {
    if (!relay) return;
    const relayOrigin = new URL(relay).origin;
    const onMessage = (e: MessageEvent) => {
      if (e.origin !== relayOrigin || !isRelayMessage(e.data)) return;
      const msg = e.data;
      if (msg.event === "ready") handlers.current.onReady?.();
      else if (msg.event === "state") handlers.current.onStateChange?.(msg.info);
      else handlers.current.onError?.(msg.info);
    };
    window.addEventListener("message", onMessage);
    return () => window.removeEventListener("message", onMessage);
  }, [relay]);

  return (
    <iframe
      ref={frameRef}
      key={src}
      src={src}
      title={title}
      referrerPolicy="strict-origin-when-cross-origin"
      allow="autoplay; encrypted-media; picture-in-picture; fullscreen"
      allowFullScreen
      className={cn("aspect-video min-h-[200px] min-w-[356px] rounded-card bg-black", className)}
    />
  );
}
