import { Blocks } from "lucide-react";
import { useState } from "react";
import { Icon } from "@/components/ui";
import { cn } from "@/lib/cn";

/** An extension's own icon (a `data:` URL from its package), or a quiet fallback. */
export function ExtensionIcon({ src, className }: { src: string | null; className?: string }) {
  const [failed, setFailed] = useState(false);
  if (!src || failed) {
    return (
      <div
        aria-hidden="true"
        className={cn(
          "flex shrink-0 items-center justify-center rounded-[10px] bg-fill-tertiary text-text-tertiary",
          className,
        )}
      >
        <Icon as={Blocks} size={16} className="size-[45%]" />
      </div>
    );
  }
  return (
    <img
      src={src}
      alt=""
      draggable={false}
      onError={() => setFailed(true)}
      className={cn("shrink-0 rounded-[10px] bg-fill-tertiary object-cover", className)}
    />
  );
}
