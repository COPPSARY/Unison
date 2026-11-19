import { X } from "lucide-react";
import type { ReactNode } from "react";
import { strings } from "@/i18n";
import { cn } from "@/lib/cn";
import { Icon } from "./Icon";
import { IconButton } from "./IconButton";

export type BannerTone = "info" | "warning" | "danger";

export type BannerProps = {
  tone?: BannerTone;
  /** What happened, one line. */
  children: ReactNode;
  /** The one next step, e.g. a Reconnect button. */
  action?: ReactNode;
  icon?: ReactNode;
  onDismiss?: () => void;
};

const dot: Record<BannerTone, string> = {
  info: "bg-text-tertiary",
  warning: "bg-warning",
  danger: "bg-danger",
};

/** App-wide problem strip (offline, quota exhausted, token expired): a floating glass capsule. */
export function Banner({ tone = "info", children, action, icon, onDismiss }: BannerProps) {
  return (
    <div
      role={tone === "info" ? "status" : "alert"}
      className="glass flex min-h-10 items-center gap-3 rounded-full py-1 pr-1.5 pl-4"
    >
      <span aria-hidden="true" className={cn("size-2 shrink-0 rounded-full", dot[tone])} />
      {icon ? <span className="text-text-muted">{icon}</span> : null}
      <p className="flex-1 text-body text-text">{children}</p>
      {action}
      {onDismiss ? (
        <IconButton label={strings.common.dismiss} size="sm" icon={<Icon as={X} />} onClick={onDismiss} />
      ) : null}
    </div>
  );
}
