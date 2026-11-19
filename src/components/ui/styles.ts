/** Shared class fragments so every control behaves the same (ADR 0002). */

/** Hover/press feedback: tone change on hover, snappy-spring scale on press. */
export const pressable = "u-press";
/** Tone-only transition for non-pressable controls (fields, rows). */
export const tone = "u-tone";

/** Glass surface for floating overlays (menus, popovers, toasts, dialogs). */
export const overlayGlass = "glass-thick rounded-panel text-text";

/** Text field look: filled, borderless, accent focus ring. */
export const fieldBase =
  "h-8 w-full rounded-control bg-fill-tertiary px-3 text-body text-text outline-none " +
  "placeholder:text-text-tertiary hover:bg-fill-secondary " +
  "disabled:cursor-not-allowed disabled:bg-fill-quaternary disabled:text-text-disabled " +
  "aria-[invalid=true]:shadow-[0_0_0_1.5px_var(--danger)]";

/** Highlighted row inside menus and selects (macOS style: accent fill). */
export const menuItem =
  "relative flex h-7 cursor-default items-center gap-2 rounded-[8px] px-2 text-body outline-none " +
  "select-none data-highlighted:bg-accent data-highlighted:text-accent-foreground " +
  "data-disabled:text-text-disabled";
