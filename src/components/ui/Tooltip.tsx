import * as RTooltip from "@radix-ui/react-tooltip";
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

export const TooltipProvider = RTooltip.Provider;

export type TooltipProps = {
  content: ReactNode;
  children: ReactNode;
  side?: "top" | "right" | "bottom" | "left";
};

export function Tooltip({ content, children, side = "top" }: TooltipProps) {
  return (
    <RTooltip.Root>
      <RTooltip.Trigger asChild>{children}</RTooltip.Trigger>
      <RTooltip.Portal>
        <RTooltip.Content
          side={side}
          sideOffset={8}
          className={cn(
            "glass-thick u-anim-pop z-50 max-w-64 rounded-[10px] px-2.5 py-1.5",
            "text-callout text-text",
          )}
        >
          {content}
        </RTooltip.Content>
      </RTooltip.Portal>
    </RTooltip.Root>
  );
}
