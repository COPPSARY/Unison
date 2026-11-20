import type { SourceKind } from "@/lib/bindings";

const KINDS: readonly string[] = ["local", "audius", "extension"] satisfies SourceKind[];

/** A stored provider string as a source kind (`null` for anything else, e.g. Unison playlists). */
export function sourceKind(provider: string | null | undefined): SourceKind | null {
  return provider && KINDS.includes(provider) ? (provider as SourceKind) : null;
}
