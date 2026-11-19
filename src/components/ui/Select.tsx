import * as RSelect from "@radix-ui/react-select";
import { Check, ChevronsUpDown } from "lucide-react";
import { cn } from "@/lib/cn";
import { Icon } from "./Icon";
import { fieldBase, menuItem, overlayGlass, tone } from "./styles";

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
          tone,
          "inline-flex items-center justify-between gap-2 data-placeholder:text-text-tertiary",
          className,
        )}
      >
        <RSelect.Value {...(placeholder === undefined ? {} : { placeholder })} />
        <RSelect.Icon className="text-text-muted">
          <Icon as={ChevronsUpDown} size={12} />
        </RSelect.Icon>
      </RSelect.Trigger>
      <RSelect.Portal>
        <RSelect.Content
          position="popper"
          sideOffset={6}
          className={cn(
            overlayGlass,
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
                className={cn(menuItem, "pr-2 pl-7")}
              >
                <RSelect.ItemIndicator className="absolute left-2">
                  <Icon as={Check} size={12} />
                </RSelect.ItemIndicator>
                <RSelect.ItemText>{o.label}</RSelect.ItemText>
              </RSelect.Item>
            ))}
          </RSelect.Viewport>
        </RSelect.Content>
      </RSelect.Portal>
    </RSelect.Root>
  );
}
