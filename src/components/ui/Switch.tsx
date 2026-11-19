import { cn } from "@/lib/cn";

/** On/off switch (capsule track, accent when on). Tone transition only; no lift. */
export function Switch({
  checked,
  onCheckedChange,
  label,
  disabled = false,
}: {
  checked: boolean;
  onCheckedChange: (checked: boolean) => void;
  /** Accessible name (the row label). */
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onCheckedChange(!checked)}
      className={cn(
        "u-tone relative inline-flex h-[22px] w-[38px] shrink-0 items-center rounded-full p-0.5 outline-none",
        "focus-visible:shadow-[0_0_0_3px_var(--focus-ring)] disabled:opacity-50",
        checked ? "bg-accent" : "bg-fill",
      )}
    >
      <span
        aria-hidden="true"
        className={cn(
          "size-[18px] rounded-full bg-thumb shadow-[0_1px_3px_rgb(0_0_0/0.25)] transition-transform",
          "duration-[var(--spring-snappy-duration)] ease-[var(--spring-snappy)]",
          checked ? "translate-x-4" : "translate-x-0",
        )}
      />
    </button>
  );
}
