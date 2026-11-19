/** Shared class fragments so every control behaves the same. */
export const toneTransition =
  "transition-[background-color,color,border-color] duration-(--duration-hover) ease-out";

export const focusRing =
  "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent";

export const overlaySurface =
  "rounded-panel bg-surface-raised text-text shadow-overlay";

export const fieldBase =
  "h-8 w-full rounded-control border border-border-strong bg-surface px-3 text-sm text-text " +
  "placeholder:text-text-muted hover:border-text-disabled disabled:cursor-not-allowed " +
  "disabled:text-text-disabled disabled:hover:border-border-strong " +
  "aria-[invalid=true]:border-danger";
