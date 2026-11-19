import type { ComponentProps } from "react";
import { cn } from "@/lib/cn";
import { focusRing, toneTransition } from "./styles";

export type ButtonVariant = "primary" | "secondary" | "ghost" | "danger";
export type ButtonSize = "sm" | "md";

const variants: Record<ButtonVariant, string> = {
  primary: "bg-accent text-accent-foreground hover:bg-accent/90",
  secondary:
    "border border-border-strong bg-surface text-text hover:bg-surface-raised",
  ghost: "text-text-muted hover:bg-surface-raised hover:text-text",
  danger: "border border-border-strong bg-surface text-danger hover:bg-surface-raised",
};

const sizes: Record<ButtonSize, string> = {
  sm: "h-7 gap-1 px-2 text-xs",
  md: "h-8 gap-2 px-3 text-sm",
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
        "inline-flex shrink-0 items-center justify-center rounded-control font-medium select-none",
        "active:scale-[0.98] disabled:pointer-events-none disabled:opacity-40",
        toneTransition,
        focusRing,
        variants[variant],
        sizes[size],
        className,
      )}
      {...props}
    />
  );
}
