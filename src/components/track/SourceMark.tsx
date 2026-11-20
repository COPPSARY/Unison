import type { SourceKind } from "@/lib/bindings";
import { strings } from "@/i18n";
import { cn } from "@/lib/cn";

/**
 * Monochrome 12 px source badge (Plan: "Source badges", text-muted). Simplified marks that
 * name the platform; brand colours are never used in Unison's chrome.
 */
export function SourceMark({ provider, className }: { provider: SourceKind; className?: string | undefined }) {
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
      {provider === "local" ? (
        <>
          <rect x="1" y="2" width="10" height="7" rx="1.5" fill="currentColor" />
          <rect x="4" y="9.5" width="4" height="1.2" rx="0.6" fill="currentColor" />
        </>
      ) : provider === "extension" ? (
        <path d="M4.5 1.5h3v2a1 1 0 1 0 2 0v-2h1a1 1 0 0 1 1 1v8a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1v-8a1 1 0 0 1 1-1z" fill="currentColor" />
      ) : (
        <path d="M6 1.2 11 10.2H1z" fill="currentColor" />
      )}
    </svg>
  );
}

/** One mark per distinct provider. */
export function SourceMarks({ providers }: { providers: SourceKind[] }) {
  const unique = [...new Set(providers)];
  return (
    <span className="inline-flex items-center gap-1">
      {unique.map((p) => (
        <SourceMark key={p} provider={p} />
      ))}
    </span>
  );
}
