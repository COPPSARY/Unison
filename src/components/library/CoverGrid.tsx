import { Play } from "lucide-react";
import type { ReactNode } from "react";
import { Link } from "react-router";
import { Artwork } from "@/components/track/Artwork";
import { SourceMark } from "@/components/track/SourceMark";
import { Icon } from "@/components/ui";
import { format, strings } from "@/i18n";
import type { ProviderId } from "@/lib/bindings";
import { cn } from "@/lib/cn";

export type CoverItem = {
  id: string;
  href: string;
  title: string;
  /** One line of metadata. */
  meta: ReactNode;
  artwork: string | null;
  provider: ProviderId | null;
  kind: "album" | "artist" | "playlist";
};

/** 180 px cover grid (spec §3 Albums / Artists / Playlists index). The image is the identifier. */
export function CoverGrid({ items, onPlay }: { items: CoverItem[]; onPlay?: (item: CoverItem) => void }) {
  return (
    <ul aria-label={strings.library.grid} className="grid grid-cols-[repeat(auto-fill,minmax(180px,1fr))] gap-x-4 gap-y-6">
      {items.map((item) => {
        const round = item.kind === "artist";
        return (
          <li key={item.id} className="group relative min-w-0">
            <Link
              to={item.href}
              className="u-tone -m-2 flex flex-col gap-2 rounded-panel p-2 hover:bg-fill-quaternary"
            >
              <div className="relative">
                <Artwork src={item.artwork} kind={item.kind} round={round} className="aspect-square w-full" />
              </div>
              <div className={cn("min-w-0", round && "text-center")}>
                <p className="truncate text-body font-medium text-text">{item.title}</p>
                <p className={cn("flex items-center gap-1.5 truncate text-callout text-text-muted", round && "justify-center")}>
                  {item.provider ? <SourceMark provider={item.provider} /> : null}
                  <span className="truncate">{item.meta}</span>
                </p>
              </div>
            </Link>
            {onPlay && !round ? (
              <button
                type="button"
                aria-label={format(strings.library.playTrack, { title: item.title })}
                onClick={() => onPlay(item)}
                className="u-press absolute right-4 bottom-[76px] flex size-10 items-center justify-center rounded-full bg-accent text-accent-foreground opacity-0 shadow-glass group-hover:opacity-100 focus-visible:opacity-100"
              >
                <Icon as={Play} size={16} fill="currentColor" />
              </button>
            ) : null}
          </li>
        );
      })}
    </ul>
  );
}
