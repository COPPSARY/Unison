import * as ToggleGroup from "@radix-ui/react-toggle-group";
import { useLayoutEffect, useRef, useState } from "react";
import { cn } from "@/lib/cn";
import { tone } from "./styles";

export type SegmentedOption<T extends string> = { value: T; label: string; disabled?: boolean };

export type SegmentedProps<T extends string> = {
  value: T;
  onValueChange: (value: T) => void;
  options: Array<SegmentedOption<T>>;
  label: string;
  disabled?: boolean;
};

type Thumb = { left: number; width: number };

/** Capsule segmented control; the selected thumb slides with the smooth spring. */
export function Segmented<T extends string>({
  value,
  onValueChange,
  options,
  label,
  disabled = false,
}: SegmentedProps<T>) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [thumb, setThumb] = useState<Thumb | null>(null);

  useLayoutEffect(() => {
    const root = rootRef.current;
    const active = Array.from(root?.querySelectorAll<HTMLElement>("[data-value]") ?? []).find(
      (el) => el.dataset["value"] === value,
    );
    if (!root || !active) return;
    const measure = () => setThumb({ left: active.offsetLeft, width: active.offsetWidth });
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(root);
    return () => ro.disconnect();
  }, [value, options]);

  return (
    <ToggleGroup.Root
      ref={rootRef}
      type="single"
      aria-label={label}
      value={value}
      disabled={disabled}
      onValueChange={(next) => {
        // Radix emits "" when the active item is clicked again; keep one selected.
        const match = options.find((o) => o.value === next);
        if (match) onValueChange(match.value);
      }}
      className={cn(
        "relative inline-flex h-8 items-center rounded-full bg-fill-tertiary p-0.5",
        disabled && "opacity-50",
      )}
    >
      {thumb ? (
        <span
          aria-hidden="true"
          className="absolute top-0.5 bottom-0.5 rounded-full bg-thumb shadow-[0_1px_3px_rgb(0_0_0/0.2)]"
          style={{
            left: thumb.left,
            width: thumb.width,
            transition:
              "left var(--spring-smooth-duration) var(--spring-smooth), width var(--spring-smooth-duration) var(--spring-smooth)",
          }}
        />
      ) : null}
      {options.map((o) => (
        <ToggleGroup.Item
          key={o.value}
          value={o.value}
          data-value={o.value}
          {...(o.disabled === undefined ? {} : { disabled: o.disabled })}
          className={cn(
            "relative z-10 h-full rounded-full px-3.5 text-callout font-medium text-text-muted",
            "hover:text-text data-[state=on]:text-text disabled:pointer-events-none",
            tone,
          )}
        >
          {o.label}
        </ToggleGroup.Item>
      ))}
    </ToggleGroup.Root>
  );
}
