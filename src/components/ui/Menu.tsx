import * as DM from "@radix-ui/react-dropdown-menu";
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";
import { menuItem, overlayGlass } from "./styles";

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
        sideOffset={6}
        className={cn(overlayGlass, "u-anim-pop z-50 min-w-52 p-1")}
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
      className={cn(menuItem, "group", danger ? "text-danger" : "text-text")}
    >
      {icon ? (
        <span className="text-text-muted group-data-highlighted:text-accent-foreground">{icon}</span>
      ) : null}
      <span className="flex-1">{children}</span>
      {shortcut ? (
        <span className="text-callout text-text-tertiary tabular-nums group-data-highlighted:text-accent-foreground">
          {shortcut}
        </span>
      ) : null}
    </DM.Item>
  );
}

export function MenuSeparator() {
  return <DM.Separator className="mx-2 my-1 h-px bg-border" />;
}
