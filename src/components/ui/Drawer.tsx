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

/** Right-side panel for multi-step flows (e.g. Spotify BYOK onboarding). */
export function DrawerContent({ title, description, children, footer }: DrawerContentProps) {
  return (
    <RDialog.Portal>
      <Overlay />
      <RDialog.Content
        {...(description ? {} : { "aria-describedby": undefined })}
        className={cn(
          "u-anim-drawer fixed top-0 right-0 bottom-0 z-50 flex w-[min(420px,100vw)] flex-col",
          "border-l border-border bg-surface shadow-overlay focus-visible:outline-none",
        )}
      >
        <div className="flex items-start justify-between gap-4 border-b border-border px-6 py-4">
          <div>
            <RDialog.Title className="font-display text-base font-medium text-text">
              {title}
            </RDialog.Title>
            {description ? (
              <RDialog.Description className="mt-1 text-sm text-text-muted">
                {description}
              </RDialog.Description>
            ) : null}
          </div>
          <RDialog.Close asChild>
            <IconButton label={strings.common.close} size="sm" icon={<Icon as={X} />} className="-mr-2" />
          </RDialog.Close>
        </div>
        <div className="flex-1 overflow-y-auto px-6 py-6">{children}</div>
        {footer ? (
          <div className="flex justify-end gap-2 border-t border-border px-6 py-4">{footer}</div>
        ) : null}
      </RDialog.Content>
    </RDialog.Portal>
  );
}
