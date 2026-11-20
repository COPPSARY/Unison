import * as RDialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { strings } from "@/i18n";
import { cn } from "@/lib/cn";
import { Overlay } from "./Dialog";
import { Icon } from "./Icon";
import { IconButton } from "./IconButton";

export const Drawer = RDialog.Root;
export const DrawerTrigger = RDialog.Trigger;
export const DrawerClose = RDialog.Close;

export type DrawerContentProps = {
  title: string;
  description?: string;
  children: ReactNode;
  footer?: ReactNode;
};

/** Floating glass sheet inset from the window edge. */
export function DrawerContent({ title, description, children, footer }: DrawerContentProps) {
  return (
    <RDialog.Portal>
      <Overlay />
      <RDialog.Content
        {...(description ? {} : { "aria-describedby": undefined })}
        className={cn(
          "glass-thick u-anim-sheet fixed top-3 right-3 bottom-3 z-50 flex w-[min(400px,calc(100vw-24px))]",
          "flex-col overflow-hidden rounded-sheet text-text",
        )}
      >
        <div className="flex items-start justify-between gap-4 px-5 pt-5 pb-3">
          <div>
            <RDialog.Title className="text-title2">{title}</RDialog.Title>
            {description ? (
              <RDialog.Description className="mt-1 text-callout text-text-muted">
                {description}
              </RDialog.Description>
            ) : null}
          </div>
          <RDialog.Close asChild>
            <IconButton label={strings.common.close} size="sm" icon={<Icon as={X} />} className="-mr-1" />
          </RDialog.Close>
        </div>
        <div className="flex-1 overflow-y-auto px-5 py-3">{children}</div>
        {footer ? <div className="flex justify-end gap-2 px-5 pt-3 pb-5">{footer}</div> : null}
      </RDialog.Content>
    </RDialog.Portal>
  );
}
