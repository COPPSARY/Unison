import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/cn";
import { fieldBase, focusRing, toneTransition } from "./styles";

export type InputProps = ComponentProps<"input"> & {
  invalid?: boolean;
  /** Optional leading icon (rendered in text-muted). */
  leading?: ReactNode;
};

export function Input({ invalid = false, leading, className, ...props }: InputProps) {
  const field = (
    <input
      aria-invalid={invalid || undefined}
      className={cn(fieldBase, toneTransition, focusRing, leading ? "pl-8" : "", className)}
      {...props}
    />
  );
  if (!leading) return field;
  return (
    <div className="relative w-full">
      <span className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-text-muted">
        {leading}
      </span>
      {field}
    </div>
  );
}
