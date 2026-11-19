import * as RDialog from "@radix-ui/react-dialog";
import type { ReactNode } from "react";
import { cn } from "@/lib/cn";

export const Dialog = RDialog.Root;
export const DialogTrigger = RDialog.Trigger;
export const DialogClose = RDialog.Close;

export type DialogContentProps = {
  title: string;
  /** One line that prevents a mistake; omit when the title says enough. */
  description?: string;
  /** Optional icon shown above the title (label colour, never on a tinted shape). */
  icon?: ReactNode;
  children?: ReactNode;
  /** Action buttons; they share the width. Put the primary/danger action last. */
  footer?: ReactNode;
};

export function Overlay() {
  return <RDialog.Overlay className="u-anim-overlay fixed inset-0 z-40 bg-scrim" />;
}

/** Alert-style glass dialog: centred, compact, capsule actions that share the width. */
export function DialogContent({ title, description, icon, children, footer }: DialogContentProps) {
  return (
    <RDialog.Portal>
      <Overlay />
      <RDialog.Content
        {...(description ? {} : { "aria-describedby": undefined })}
        className={cn(
          "glass-thick u-anim-dialog fixed top-1/2 left-1/2 z-50 w-[min(360px,calc(100vw-64px))]",
          "-translate-x-1/2 -translate-y-1/2 rounded-sheet p-5 text-center text-text",
        )}
      >
        {icon ? <div className="mb-3 flex justify-center text-text-muted">{icon}</div> : null}
        <RDialog.Title className="text-title3">{title}</RDialog.Title>
        {description ? (
          <RDialog.Description className="mt-1.5 text-callout text-text-muted">
            {description}
          </RDialog.Description>
        ) : null}
        {children ? <div className="mt-4 text-left">{children}</div> : null}
        {footer ? <div className="mt-5 flex gap-2 *:flex-1">{footer}</div> : null}
      </RDialog.Content>
    </RDialog.Portal>
  );
}
