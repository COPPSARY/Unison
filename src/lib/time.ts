import { strings } from "@/i18n";

const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });

const STEPS: Array<[Intl.RelativeTimeFormatUnit, number]> = [
  ["year", 365 * 24 * 3600],
  ["month", 30 * 24 * 3600],
  ["week", 7 * 24 * 3600],
  ["day", 24 * 3600],
  ["hour", 3600],
  ["minute", 60],
];

/** "5 minutes ago" for a Unix-seconds timestamp; "just now" under a minute. */
export function relativeTime(unixSeconds: number, nowMs: number = Date.now()): string {
  const diff = Math.round(unixSeconds - nowMs / 1000);
  const abs = Math.abs(diff);
  for (const [unit, seconds] of STEPS) {
    if (abs >= seconds) return rtf.format(Math.round(diff / seconds), unit);
  }
  return strings.common.justNow;
}
