import * as RToast from "@radix-ui/react-toast";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import { strings } from "@/i18n";
import { cn } from "@/lib/cn";
import { useToastStore, type ToastItem } from "@/stores/toast";
import { Button } from "./Button";
import { Icon } from "./Icon";
import { IconButton } from "./IconButton";
import { overlaySurface } from "./styles";

const DURATION_MS = 5000;

export function ToastCard({
  item,
  onOpenChange,
}: {
  item: ToastItem;
  onOpenChange?: (open: boolean) => void;
}) {
  return (
    <RToast.Root
      duration={DURATION_MS}
      {...(onOpenChange ? { onOpenChange } : {})}
      className={cn(overlaySurface, "u-anim-toast flex w-80 items-start gap-3 p-3")}
    >
      <div className="min-w-0 flex-1">
        <RToast.Title
          className={cn("text-sm font-medium", item.tone === "danger" ? "text-danger" : "text-text")}
        >
          {item.title}
        </RToast.Title>
        {item.description ? (
          <RToast.Description className="mt-1 text-sm text-text-muted">
            {item.description}
          </RToast.Description>
        ) : null}
      </div>
      {item.action ? (
        <RToast.Action altText={item.action.label} asChild>
          <Button size="sm" variant="ghost" onClick={item.action.onAction}>
            {item.action.label}
          </Button>
        </RToast.Action>
      ) : null}
      <RToast.Close asChild>
        <IconButton label={strings.common.dismiss} size="sm" icon={<Icon as={X} />} />
      </RToast.Close>
    </RToast.Root>
  );
}

/** Mount once at the app root. Toasts sit bottom-right, above the 72 px player bar. */
export function Toaster({ children }: { children?: ReactNode }) {
  const toasts = useToastStore((s) => s.toasts);
  const dismiss = useToastStore((s) => s.dismiss);
  return (
    <RToast.Provider swipeDirection="right" label={strings.common.notifications}>
      {children}
      {toasts.map((t) => (
        <ToastCard key={t.id} item={t} onOpenChange={(open) => !open && dismiss(t.id)} />
      ))}
      <RToast.Viewport className="fixed right-4 bottom-[88px] z-50 flex flex-col gap-2 outline-none" />
    </RToast.Provider>
  );
}
