import type { ProviderId } from "@/lib/bindings";
import { strings } from "@/i18n";
import { cn } from "@/lib/cn";

/**
 * Monochrome 12 px source badge (Plan: "Source badges", text-muted). Simplified marks that
 * name the platform; brand colours are never used in Unison's chrome.
 */
export function SourceMark({ provider, className }: { provider: ProviderId; className?: string }) {
  const label = strings.source[provider];
  return (
    <svg
      role="img"
      aria-label={label}
      viewBox="0 0 12 12"
      width={12}
      height={12}
      className={cn("shrink-0 text-text-muted", className)}
    >
      <title>{label}</title>
      {provider === "spotify" ? (
        <>
          <circle cx="6" cy="6" r="5.5" fill="currentColor" />
          <g fill="none" stroke="var(--background)" strokeLinecap="round">
            <path d="M3.1 4.6c2-.6 4.1-.4 5.9.6" strokeWidth="1.1" />
            <path d="M3.5 6.5c1.6-.45 3.2-.3 4.6.45" strokeWidth="0.95" />
            <path d="M3.9 8.2c1.2-.3 2.4-.2 3.4.35" strokeWidth="0.8" />
          </g>
        </>
      ) : (
        <>
          <rect x="0.5" y="2" width="11" height="8" rx="2.4" fill="currentColor" />
          <path d="M5 4.4v3.2L7.8 6z" fill="var(--background)" />
        </>
      )}
    </svg>
  );
}

/** One mark per distinct provider. */
export function SourceMarks({ providers }: { providers: ProviderId[] }) {
  const unique = [...new Set(providers)];
  return (
    <span className="inline-flex items-center gap-1">
      {unique.map((p) => (
        <SourceMark key={p} provider={p} />
      ))}
    </span>
  );
}
