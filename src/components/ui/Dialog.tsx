import * as RDialog from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { strings } from "@/i18n";
import { cn } from "@/lib/cn";
import { Icon } from "./Icon";
import { IconButton } from "./IconButton";
import { overlaySurface } from "./styles";

export const Dialog = RDialog.Root;
export const DialogTrigger = RDialog.Trigger;
export const DialogClose = RDialog.Close;

export type DialogContentProps = {
  title: string;
  /** One line that prevents a mistake; omit when the title says enough. */
  description?: string;
  children?: ReactNode;
  /** Action buttons, right-aligned. Put the primary/danger action last. */
  footer?: ReactNode;
};

export function Overlay() {
  return <RDialog.Overlay className="u-anim-overlay fixed inset-0 z-40 bg-scrim" />;
}

export function DialogContent({ title, description, children, footer }: DialogContentProps) {
  return (
    <RDialog.Portal>
      <Overlay />
      <RDialog.Content
        {...(description ? {} : { "aria-describedby": undefined })}
        className={cn(
          overlaySurface,
          "u-anim-dialog fixed top-1/2 left-1/2 z-50 w-[min(440px,calc(100vw-64px))]",
          "-translate-x-1/2 -translate-y-1/2 p-6 focus-visible:outline-none",
        )}
      >
        <div className="flex items-start justify-between gap-4">
          <RDialog.Title className="font-display text-base font-medium text-text">
            {title}
          </RDialog.Title>
          <RDialog.Close asChild>
            <IconButton label={strings.common.close} size="sm" icon={<Icon as={X} />} className="-mt-1 -mr-2" />
          </RDialog.Close>
        </div>
        {description ? (
          <RDialog.Description className="mt-2 text-sm text-text-muted">
            {description}
          </RDialog.Description>
        ) : null}
        {children ? <div className="mt-4">{children}</div> : null}
        {footer ? <div className="mt-6 flex justify-end gap-2">{footer}</div> : null}
      </RDialog.Content>
    </RDialog.Portal>
  );
}
