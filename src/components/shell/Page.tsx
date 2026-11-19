import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";
import { Icon } from "@/components/ui";

/** Page wrapper: large title, optional toolbar, content. */
export function Page({
  title,
  toolbar,
  children,
}: {
  title: string;
  toolbar?: ReactNode;
  children?: ReactNode;
}) {
  return (
    <div className="flex min-h-full flex-col px-8 pb-8">
      <div className="flex items-end justify-between gap-4 pt-4 pb-6">
        <h1 className="text-large-title text-text">{title}</h1>
        {toolbar}
      </div>
      {children}
    </div>
  );
}

/** Centred empty state: muted icon (no tinted shape), title, one line, optional action. */
export function EmptyState({
  icon,
  title,
  body,
  action,
}: {
  icon: LucideIcon;
  title?: string;
  body: string;
  action?: ReactNode;
}) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-2 py-16 text-center">
      <span className="mb-1 text-text-tertiary">
        <Icon as={icon} size={20} className="size-8" />
      </span>
      {title ? <p className="text-title3 text-text">{title}</p> : null}
      <p className="max-w-sm text-body text-text-muted">{body}</p>
      {action ? <div className="mt-3">{action}</div> : null}
    </div>
  );
}
