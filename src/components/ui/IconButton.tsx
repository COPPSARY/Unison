import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/cn";
import { pressable } from "./styles";

export type IconButtonProps = Omit<ComponentProps<"button">, "children"> & {
  /** Accessible name; icon buttons have no visible label. */
  label: string;
  icon: ReactNode;
  /** `primary` is reserved for the single play/pause control; `glass` floats over artwork. */
  variant?: "ghost" | "primary" | "glass";
  size?: "sm" | "md" | "lg" | "xl";
  active?: boolean;
};

const sizes = { sm: "size-7", md: "size-8", lg: "size-10", xl: "size-12" } as const;

const variants = {
  ghost: "hover:bg-fill-tertiary hover:text-text",
  primary: "bg-accent text-accent-foreground hover:brightness-110",
  glass: "glass-clear glass-flat text-text hover:bg-fill-tertiary",
} as const;

export function IconButton({
  label,
  icon,
  variant = "ghost",
  size = "md",
  active = false,
  className,
  type = "button",
  ...props
}: IconButtonProps) {
  return (
    <button
      type={type}
      aria-label={label}
      aria-pressed={active || undefined}
      className={cn(
        "inline-flex shrink-0 items-center justify-center rounded-full select-none",
        "disabled:pointer-events-none disabled:opacity-40",
        pressable,
        variants[variant],
        variant === "ghost" && (active ? "text-text" : "text-text-muted"),
        sizes[size],
        className,
      )}
      {...props}
    >
      {icon}
    </button>
  );
}
