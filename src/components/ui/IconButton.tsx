import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/cn";
import { focusRing, toneTransition } from "./styles";

export type IconButtonProps = Omit<ComponentProps<"button">, "children"> & {
  /** Accessible name; icon buttons have no visible label. */
  label: string;
  icon: ReactNode;
  /** `primary` is reserved for the single play/pause control. */
  variant?: "ghost" | "primary";
  size?: "sm" | "md" | "lg";
  active?: boolean;
};

const sizes = { sm: "size-7", md: "size-8", lg: "size-10" } as const;

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
        "inline-flex shrink-0 items-center justify-center select-none active:scale-[0.98]",
        "disabled:pointer-events-none disabled:opacity-40",
        toneTransition,
        focusRing,
        variant === "primary"
          ? "rounded-full bg-accent text-accent-foreground hover:bg-accent/90"
          : cn(
              "rounded-control hover:bg-surface-raised hover:text-text",
              active ? "text-text" : "text-text-muted",
            ),
        sizes[size],
        className,
      )}
      {...props}
    >
      {icon}
    </button>
  );
}
