import { strings } from "@/i18n";
import type { SpotifyCapabilities_Serialize as Caps } from "@/lib/bindings";

const t = strings.settings.spotifyPlayback;

export type Status = { label: string; dot: string };

/** Ready / Needs Premium / No device / Unknown (addendum §5 Playback statuses). */
export function spotifyStatus(caps: Caps | undefined, connected: boolean): Status {
  if (!connected) return { label: t.notConnected, dot: "bg-text-tertiary" };
  if (!caps) return { label: "", dot: "bg-text-tertiary" };
  if (caps.premium === false) return { label: t.needsPremium, dot: "bg-warning" };
  const usable = caps.devices.filter((d) => d.id && !d.isRestricted);
  if (usable.length === 0) return { label: t.noDevice, dot: "bg-warning" };
  if (caps.premium === null) return { label: t.unknown, dot: "bg-text-tertiary" };
  return { label: t.ready, dot: "bg-success" };
}
