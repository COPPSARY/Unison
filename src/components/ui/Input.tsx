import type { ComponentProps, ReactNode } from "react";
import { cn } from "@/lib/cn";
import { fieldBase, tone } from "./styles";

export type InputProps = ComponentProps<"input"> & {
  invalid?: boolean;
  /** Optional leading icon (rendered in text-muted). */
  leading?: ReactNode;
  /** Capsule search-field shape. */
  shape?: "field" | "capsule";
};

export function Input({
  invalid = false,
  leading,
  shape = "field",
  className,
  ...props
}: InputProps) {
  const field = (
    <input
      aria-invalid={invalid || undefined}
      className={cn(
        fieldBase,
        tone,
        shape === "capsule" && "rounded-full",
        leading ? "pl-8" : "",
        className,
      )}
      {...props}
    />
  );
  if (!leading) return field;
  return (
    <div className="relative w-full">
      <span className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-text-muted">
        {leading}
      </span>
      {field}
    </div>
  );
}
