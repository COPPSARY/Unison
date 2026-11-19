import type { LucideIcon, LucideProps } from "lucide-react";

/** Lucide icon at the house stroke width (1.5 px). Sizes: 12 badge, 16 list, 20 nav/player. */
export function Icon({
  as: Glyph,
  size = 16,
  ...props
}: Omit<LucideProps, "ref"> & { as: LucideIcon; size?: 12 | 16 | 20 }) {
  return <Glyph size={size} strokeWidth={1.5} aria-hidden="true" {...props} />;
}
