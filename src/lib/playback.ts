import { useEffect } from "react";
import { format, strings } from "@/i18n";
import { commands, events, type SourceKind, type Track_Serialize as Track } from "@/lib/bindings";
import { errorMessage, isAppError } from "@/lib/errors";
import { isTauri } from "@/lib/window";
import { usePlayerStore } from "@/stores/player";
import { toast } from "@/stores/toast";

const sourceName = (k: SourceKind) => strings.source[k];

/**
 * Plays `tracks` starting at `index` (Rust resolves sources). On "no playable source" it offers
 * Open in Spotify / YouTube when the track has such a link.
 */
export async function playTracks(tracks: Track[], index: number, shuffle = false): Promise<void> {
  if (!isTauri() || tracks.length === 0) return;
  const ids = tracks.map((t) => t.id);
  try {
    if (shuffle) await commands.playerControl({ type: "shuffle", on: true });
    const start = shuffle ? Math.floor(Math.random() * ids.length) : index;
    await commands.playerPlay(ids, start, null);
  } catch (err: unknown) {
    const track = tracks[index];
    const external = track?.sources.find((s) => s.provider === "spotify" || s.provider === "youtube");
    const noSource = isAppError(err) && err.code.type === "noPlayableSource";
    toast({
      title: errorMessage(err),
      tone: "danger",
      ...(noSource && track && external
        ? {
            action: {
              label: format(strings.player.openIn, { provider: strings.source[external.provider] }),
              onAction: () => void commands.trackOpenExternal(track.id, external.provider),
            },
          }
        : {}),
    });
  }
}

/** Subscribes to player state + fallback events; loads the current state once. Mount once. */
export function usePlayerEvents() {
  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const unlisten: Array<() => void> = [];
    const keep = (p: Promise<() => void>) => void p.then((fn) => (disposed ? fn() : unlisten.push(fn)));

    // Promise.resolve: tolerate mocked bindings in tests.
    void Promise.resolve(commands.playerState())
      .then((s) => s && !disposed && usePlayerStore.getState().set(s))
      .catch(() => undefined);
    keep(events.playerState.listen(({ payload }) => usePlayerStore.getState().set(payload)));
    keep(
      events.playerFallback.listen(({ payload }) =>
        toast({
          title: format(strings.player.fallback, {
            to: sourceName(payload.toProvider),
            from: sourceName(payload.fromProvider),
          }),
        }),
      ),
    );
    return () => {
      disposed = true;
      unlisten.forEach((fn) => fn());
    };
  }, []);
}
