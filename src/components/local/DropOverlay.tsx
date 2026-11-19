import { FolderDown } from "lucide-react";
import { Icon } from "@/components/ui";
import { strings } from "@/i18n";
import { useLocalImportStore } from "@/stores/localImport";
import { useUiStore } from "@/stores/ui";

const t = strings.localImport;

/** Shown while files are dragged over the window; Rust imports them on drop (UP-11). */
export function DropOverlay() {
  const dragging = useLocalImportStore((s) => s.dragging);
  const copy = useUiStore((s) => s.settings?.local.copyOnDrop ?? false);
  if (!dragging) return null;
  return (
    <div
      data-state="open"
      data-testid="drop-overlay"
      className="u-anim-overlay pointer-events-none fixed inset-0 z-50 flex items-center justify-center bg-scrim"
    >
      <div className="glass-thick flex w-[min(360px,calc(100vw-64px))] flex-col items-center gap-2 rounded-sheet px-6 py-8 text-center">
        <Icon as={FolderDown} size={20} className="mb-1 size-8 text-text-muted" />
        <p className="text-title3 text-text">{t.dropTitle}</p>
        <p className="text-callout text-text-muted">{copy ? t.dropCopy : t.dropReference}</p>
      </div>
    </div>
  );
}
