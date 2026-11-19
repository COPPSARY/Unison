import * as DM from "@radix-ui/react-dropdown-menu";
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { overlaySurface } from "./styles";

export const Menu = DM.Root;
export const MenuTrigger = DM.Trigger;

export function MenuContent({
  children,
  align = "end",
}: {
  children: ReactNode;
  align?: "start" | "center" | "end";
}) {
  return (
    <DM.Portal>
      <DM.Content
        align={align}
        sideOffset={4}
        className={cn(overlaySurface, "u-anim-pop z-50 min-w-48 p-1")}
      >
        {children}
      </DM.Content>
    </DM.Portal>
  );
}

export type MenuItemProps = {
  children: ReactNode;
  icon?: ReactNode;
  shortcut?: string;
  danger?: boolean;
  disabled?: boolean;
  onSelect?: () => void;
};

export function MenuItem({ children, icon, shortcut, danger, disabled, onSelect }: MenuItemProps) {
  return (
    <DM.Item
      {...(disabled === undefined ? {} : { disabled })}
      {...(onSelect ? { onSelect } : {})}
      className={cn(
        "flex h-8 cursor-default items-center gap-2 rounded-control px-2 text-sm outline-none",
        "select-none data-highlighted:bg-surface data-disabled:text-text-disabled",
        danger ? "text-danger" : "text-text",
      )}
    >
      {icon ? <span className="text-text-muted">{icon}</span> : null}
      <span className="flex-1">{children}</span>
      {shortcut ? (
        <span className="font-mono text-xs text-text-muted tabular-nums">{shortcut}</span>
      ) : null}
    </DM.Item>
  );
}

export function MenuSeparator() {
  return <DM.Separator className="my-1 h-px bg-border" />;
}
