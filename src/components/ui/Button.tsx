import type { ComponentProps } from "react";
import { cn } from "@/lib/cn";
import { pressable } from "./styles";

export type ButtonVariant = "primary" | "secondary" | "glass" | "ghost" | "danger";
export type ButtonSize = "sm" | "md" | "lg";

const variants: Record<ButtonVariant, string> = {
  /** The one prominent action: accent capsule. */
  primary: "bg-accent text-accent-foreground hover:brightness-110",
  /** Default bordered-style capsule on a fill. */
  secondary: "bg-fill-secondary text-text hover:bg-fill",
  /** Over artwork or other rich backgrounds. */
  glass: "glass-clear glass-flat text-text hover:bg-fill-tertiary",
  /** Borderless. */
  ghost: "text-text-muted hover:bg-fill-tertiary hover:text-text",
  danger: "bg-fill-secondary text-danger hover:bg-fill",
};

const sizes: Record<ButtonSize, string> = {
  sm: "h-7 gap-1 px-3 text-callout",
  md: "h-8 gap-1.5 px-4 text-body",
  lg: "h-10 gap-2 px-5 text-title3 font-semibold",
};

export type ButtonProps = ComponentProps<"button"> & {
  variant?: ButtonVariant;
  size?: ButtonSize;
};

export function Button({
  variant = "secondary",
  size = "md",
  className,
  type = "button",
  ...props
}: ButtonProps) {
  return (
    <button
      type={type}
      className={cn(
        "inline-flex shrink-0 items-center justify-center rounded-full font-medium select-none",
        "disabled:pointer-events-none disabled:opacity-40",
        pressable,
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    />
  );
}
