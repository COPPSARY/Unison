import { format, strings } from "@/i18n";
import type { Capability } from "@/lib/bindings";

const t = strings.extensions;

/** Route of an extension's details page. */
export const detailsHref = (key: string) => `/extensions/${encodeURIComponent(key)}`;

/** What an extension adds to Unison. */
export function addsLine(capabilities: readonly Capability[]): string {
  if (capabilities.length === 0) return t.addsNothing;
  return format(t.adds, { what: capabilities.map((c) => (c === "search" ? t.capSearch : t.capPlay)).join(", ") });
}

/** GitHub commit (40 hex) or package SHA-256 (64 hex), shortened. */
export function pinLine(sha: string): string {
  return format(sha.length === 64 ? t.packageHash : t.commit, { sha: sha.slice(0, 12) });
}

/** A manifest `type` as a title and one line on what it means in Unison. */
export function typeInfo(type: string): { title: string; body: string } {
  switch (type) {
    case "metadata_provider":
      return { title: t.typeMetadata, body: t.typeMetadataBody };
    case "download_provider":
      return { title: t.typeDownload, body: t.typeDownloadBody };
    case "lyrics_provider":
      return { title: t.typeLyrics, body: t.typeLyricsBody };
    default:
      return { title: format(t.typeOther, { type: type.replace(/_/g, " ") }), body: t.typeOtherBody };
  }
}
