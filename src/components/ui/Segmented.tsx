import * as ToggleGroup from "@radix-ui/react-toggle-group";
import { cn } from "@/lib/cn";
import { focusRing, toneTransition } from "./styles";

export type SegmentedOption<T extends string> = { value: T; label: string; disabled?: boolean };

export type SegmentedProps<T extends string> = {
  value: T;
  onValueChange: (value: T) => void;
  options: Array<SegmentedOption<T>>;
  label: string;
  disabled?: boolean;
};

export function Segmented<T extends string>({
  value,
  onValueChange,
  options,
  label,
  disabled = false,
}: SegmentedProps<T>) {
  return (
    <ToggleGroup.Root
      type="single"
      aria-label={label}
      value={value}
      disabled={disabled}
      onValueChange={(next) => {
        // Radix emits "" when the active item is clicked again; keep one selected.
        const match = options.find((o) => o.value === next);
        if (match) onValueChange(match.value);
      }}
      className="inline-flex h-8 items-center gap-1 rounded-control border border-border p-0.5"
    >
      {options.map((o) => (
        <ToggleGroup.Item
          key={o.value}
          value={o.value}
          {...(o.disabled === undefined ? {} : { disabled: o.disabled })}
          className={cn(
            "h-full rounded-[4px] px-3 text-sm text-text-muted",
            "hover:text-text data-[state=on]:bg-surface-raised data-[state=on]:text-text",
            "disabled:pointer-events-none disabled:text-text-disabled",
            toneTransition,
            focusRing,
          )}
        >
          {o.label}
        </ToggleGroup.Item>
      ))}
    </ToggleGroup.Root>
  );
}
