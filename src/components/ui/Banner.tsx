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

const edge: Record<BannerTone, string> = {
  info: "border-l-border-strong",
  warning: "border-l-warning",
  danger: "border-l-danger",
};

/** App-wide problem strip: offline, quota exhausted, token expired. */
export function Banner({ tone = "info", children, action, icon, onDismiss }: BannerProps) {
  return (
    <div
      role={tone === "info" ? "status" : "alert"}
      className={cn(
        "flex min-h-10 items-center gap-3 border-b border-l-2 border-b-border bg-surface px-4 py-2",
        edge[tone],
      )}
    >
      {icon ? <span className="text-text-muted">{icon}</span> : null}
      <p className="flex-1 text-sm text-text">{children}</p>
      {action}
      {onDismiss ? (
        <IconButton label={strings.common.dismiss} size="sm" icon={<Icon as={X} />} onClick={onDismiss} />
      ) : null}
    </div>
  );
}
