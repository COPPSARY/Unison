import { format, strings } from "@/i18n";

const num = new Intl.NumberFormat();
const date = new Intl.DateTimeFormat(undefined, { year: "numeric", month: "short", day: "numeric" });

/** 3:42 or 1:02:03 for a track length. */
export function formatDuration(ms: number | null | undefined): string {
  if (ms === null || ms === undefined || !Number.isFinite(ms)) return "";
  const total = Math.round(ms / 1000);
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

/** "6 hr 12 min" / "42 min" for a list total. */
export function formatTotalDuration(ms: number): string {
  const minutes = Math.round(ms / 60000);
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return h > 0
    ? format(strings.library.durationHours, { h: num.format(h), m })
    : format(strings.library.durationMinutes, { m });
}

export function formatCount(count: number): string {
  return count === 1
    ? strings.library.songCountOne
    : format(strings.library.songCount, { count: num.format(count) });
}

/** Short date for "Date added" (Unix seconds). */
export function formatDate(unixSeconds: number | null | undefined): string {
  if (!unixSeconds) return "";
  return date.format(unixSeconds * 1000);
}

export const formatNumber = (n: number) => num.format(n);

const dateTime = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

/** Date and time, e.g. for a folder's last scan (Unix seconds). */
export function formatDateTime(unixSeconds: number | null | undefined): string {
  if (!unixSeconds) return "";
  return dateTime.format(unixSeconds * 1000);
}
