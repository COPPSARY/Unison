import { Disc3, ListMusic, MicVocal, Music } from "lucide-react";
import { useState } from "react";
import { Icon } from "@/components/ui";
import { cn } from "@/lib/cn";

const FALLBACK = { track: Music, album: Disc3, artist: MicVocal, playlist: ListMusic } as const;

/** Cover art with a quiet fallback. Art is shown unaltered (Spotify branding rules). */
export function Artwork({
  src,
  kind = "track",
  className,
  round = false,
}: {
  src: string | null | undefined;
  kind?: keyof typeof FALLBACK;
  className?: string | undefined;
  round?: boolean;
}) {
  const [failed, setFailed] = useState(false);
  const shape = round ? "rounded-full" : "rounded-[6px]";
  if (!src || failed) {
    return (
      <div
        aria-hidden="true"
        className={cn("flex shrink-0 items-center justify-center bg-fill-tertiary text-text-tertiary", shape, className)}
      >
        <Icon as={FALLBACK[kind]} size={16} className="size-[40%] max-h-10 max-w-10" />
      </div>
    );
  }
  return (
    <img
      src={src}
      alt=""
      loading="lazy"
      decoding="async"
      draggable={false}
      onError={() => setFailed(true)}
      className={cn("shrink-0 bg-fill-tertiary object-cover", shape, className)}
    />
  );
}

/** 2×2 mosaic of the first four covers (list headers). */
export function Mosaic({ srcs, className }: { srcs: Array<string | null>; className?: string | undefined }) {
  const four = srcs.filter((s): s is string => !!s).slice(0, 4);
  if (four.length < 4) return <Artwork src={four[0]} className={className} />;
  return (
    <div className={cn("grid shrink-0 grid-cols-2 grid-rows-2 overflow-hidden rounded-card", className)}>
      {four.map((s) => (
        <img key={s} src={s} alt="" className="size-full object-cover" draggable={false} />
      ))}
    </div>
  );
}
