import type { Song_Serialize as Song } from "@/lib/bindings";
import { commands } from "@/lib/bindings";
import { errorMessage } from "@/lib/errors";
import { toast } from "@/stores/toast";

/** Route of a song's page. */
export const songHref = (key: string) => `/track/${encodeURIComponent(key)}`;

/**
 * Plays a song from `sourceKey` (default: its first playable source) at `quality` for that
 * extension. Rust saves the song with all its sources so it can fall back.
 */
export async function playSong(song: Song, sourceKey?: string, quality: string | null = null): Promise<void> {
  const source = sourceKey ?? (song.sources.find((s) => s.canPlay) ?? song.sources[0])?.key;
  if (!source) return;
  try {
    await commands.extensionPlaySong(song.key, source, quality);
  } catch (err: unknown) {
    toast({ title: errorMessage(err), tone: "danger" });
  }
}
