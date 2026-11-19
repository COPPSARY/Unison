import * as RSelect from "@radix-ui/react-select";
import { Check, ChevronDown } from "lucide-react";
import { cn } from "@/lib/cn";
import { Icon } from "./Icon";
import { fieldBase, focusRing, overlaySurface, toneTransition } from "./styles";

export type SelectOption = { value: string; label: string; disabled?: boolean };

export type SelectProps = {
  value?: string;
  defaultValue?: string;
  onValueChange?: (value: string) => void;
  options: SelectOption[];
  placeholder?: string;
  disabled?: boolean;
  invalid?: boolean;
  /** Accessible name when no visible label is linked. */
  label: string;
  className?: string;
};

export function Select({
  options,
  placeholder,
  disabled,
  invalid = false,
  label,
  className,
  ...root
}: SelectProps) {
  return (
    <RSelect.Root {...root} {...(disabled === undefined ? {} : { disabled })}>
      <RSelect.Trigger
        aria-label={label}
        aria-invalid={invalid || undefined}
        className={cn(
          fieldBase,
          toneTransition,
          focusRing,
          "inline-flex items-center justify-between gap-2 data-placeholder:text-text-muted",
          className,
        )}
      >
        <RSelect.Value {...(placeholder === undefined ? {} : { placeholder })} />
        <RSelect.Icon className="text-text-muted">
          <Icon as={ChevronDown} />
        </RSelect.Icon>
      </RSelect.Trigger>
      <RSelect.Portal>
        <RSelect.Content
          position="popper"
          sideOffset={4}
          className={cn(
            overlaySurface,
            "u-anim-pop z-50 max-h-(--radix-select-content-available-height)",
            "min-w-(--radix-select-trigger-width) overflow-hidden p-1",
          )}
        >
          <RSelect.Viewport>
            {options.map((o) => (
              <RSelect.Item
                key={o.value}
                value={o.value}
                {...(o.disabled === undefined ? {} : { disabled: o.disabled })}
                className={cn(
                  "relative flex h-8 cursor-default items-center rounded-control pr-8 pl-2",
                  "text-sm text-text outline-none select-none data-highlighted:bg-surface",
                  "data-disabled:text-text-disabled",
                )}
              >
                <RSelect.ItemText>{o.label}</RSelect.ItemText>
                <RSelect.ItemIndicator className="absolute right-2 text-text">
                  <Icon as={Check} />
                </RSelect.ItemIndicator>
              </RSelect.Item>
            ))}
          </RSelect.Viewport>
        </RSelect.Content>
      </RSelect.Portal>
    </RSelect.Root>
  );
}
