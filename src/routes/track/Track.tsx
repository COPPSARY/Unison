import { useQueryClient } from "@tanstack/react-query";
import { ChevronLeft, Heart, Music, Play } from "lucide-react";
import { useEffect, useId, useState, type ReactNode } from "react";
import { useNavigate, useParams } from "react-router";
import { EmptyState } from "@/components/shell/Page";
import { Artwork } from "@/components/track/Artwork";
import { Button, Icon, IconButton, Select, Skeleton } from "@/components/ui";
import { format, strings } from "@/i18n";
import type { SongDetail_Serialize as SongDetail, SongSource } from "@/lib/bindings";
import { commands } from "@/lib/bindings";
import { cn } from "@/lib/cn";
import { errorMessage, isAppError } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import { useSong } from "@/lib/queries";
import { playSong } from "@/lib/songs";
import { ExtensionIcon } from "@/routes/extensions/ExtensionIcon";
import { toast } from "@/stores/toast";

const t = strings.pages.track;
type Tab = "sources" | "details";

/** One source: pick it (radio) and its quality. */
function SourceRow({
  source,
  selected,
  quality,
  onSelect,
  onQuality,
}: {
  source: SongSource;
  selected: boolean;
  quality: string | null;
  onSelect: () => void;
  onQuality: (q: string) => void;
}) {
  return (
    <li className={cn("flex items-center gap-3 px-4 py-2.5", !source.canPlay && "opacity-60")}>
      <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-3">
        <input
          type="radio"
          name="song-source"
          checked={selected}
          disabled={!source.canPlay}
          onChange={onSelect}
          aria-label={format(t.playFrom, { name: source.extensionName })}
          className="size-4 accent-(--accent)"
        />
        <ExtensionIcon src={source.icon ?? null} className="size-9" />
        <span className="min-w-0">
          <span className="block truncate text-body text-text">{source.extensionName}</span>
          {source.canPlay ? null : <span className="block text-footnote text-text-muted">{t.cantPlay}</span>}
        </span>
      </label>
      {source.qualities.length > 0 && source.canPlay ? (
        <div className="w-44">
          <Select
            label={format(t.quality, { name: source.extensionName })}
            value={quality ?? source.qualities[0]?.id ?? ""}
            onValueChange={onQuality}
            options={source.qualities.map((q) => ({ value: q.id, label: q.label }))}
          />
        </div>
      ) : (
        <span className="text-footnote text-text-tertiary">{source.canPlay ? t.defaultQuality : ""}</span>
      )}
    </li>
  );
}

function InfoRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-h-11 items-center gap-4 px-4 py-2 not-last:border-b not-last:border-border">
      <dt className="w-28 shrink-0 text-body text-text-muted">{label}</dt>
      <dd className="min-w-0 flex-1 text-body break-words text-text tabular-nums">{children}</dd>
    </div>
  );
}

function SongPage({ detail }: { detail: SongDetail }) {
  const qc = useQueryClient();
  const { song } = detail;
  const playable = song.sources.filter((s) => s.canPlay);
  const [tab, setTab] = useState<Tab>("sources");
  const [sourceKey, setSourceKey] = useState(playable[0]?.key ?? null);
  const [qualities, setQualities] = useState<Record<string, string>>({});
  const [liking, setLiking] = useState(false);
  const tabsId = useId();

  // A fresh page (another song) starts from its first playable source.
  useEffect(() => setSourceKey(playable[0]?.key ?? null), [song.key]); // eslint-disable-line react-hooks/exhaustive-deps

  const chosen = song.sources.find((s) => s.key === sourceKey) ?? null;
  const qualityOf = (s: SongSource): string | null => qualities[s.key] ?? s.quality ?? null;

  const play = () => {
    if (!chosen) return;
    // Only a quality the user picked is sent; otherwise the extension's own default applies.
    void playSong(song, chosen.key, qualities[chosen.key] ?? null).then(() =>
      qc.invalidateQueries({ queryKey: ["song", song.key] }),
    );
  };

  const like = () => {
    if (liking) return;
    setLiking(true);
    commands
      .extensionLikeSong(song.key, !detail.liked)
      .then((liked) => {
        toast({ title: liked ? t.liked : t.unliked });
        void qc.invalidateQueries({ queryKey: ["song"] });
        for (const key of ["liked", "likedStats", "albums", "artists"]) void qc.invalidateQueries({ queryKey: [key] });
      })
      .catch((err: unknown) => toast({ title: errorMessage(err), tone: "danger" }))
      .finally(() => setLiking(false));
  };

  const tabs: Array<{ id: Tab; label: string }> = [
    { id: "sources", label: t.sources },
    { id: "details", label: t.details },
  ];

  return (
    <>
      <div className="flex items-end gap-6 pt-2">
        <Artwork src={song.artwork} className="size-44 rounded-card shadow-glass" />
        <div className="flex min-w-0 flex-1 flex-col gap-3">
          <div className="flex min-w-0 flex-col gap-1">
            <h1 className="truncate text-large-title text-text">{song.title}</h1>
            <p className="truncate text-title3 text-text-muted">{song.artists.join(", ")}</p>
            <p className="truncate text-callout text-text-tertiary tabular-nums">
              {[song.album, song.durationMs ? formatDuration(song.durationMs) : null].filter(Boolean).join(" · ")}
            </p>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="primary" onClick={play} disabled={!chosen}>
              <Icon as={Play} fill="currentColor" /> {t.play}
            </Button>
            <IconButton
              label={detail.liked ? t.unlike : t.like}
              icon={<Icon as={Heart} fill={detail.liked ? "currentColor" : "none"} />}
              active={detail.liked}
              disabled={liking}
              onClick={like}
            />
          </div>
        </div>
      </div>

      <div className="flex flex-col gap-3">
        <div role="tablist" aria-label={t.tabsLabel} className="flex gap-1 border-b border-border">
          {tabs.map((x) => (
            <button
              key={x.id}
              type="button"
              role="tab"
              id={`${tabsId}-${x.id}`}
              aria-selected={tab === x.id}
              aria-controls={`${tabsId}-${x.id}-panel`}
              onClick={() => setTab(x.id)}
              className={cn(
                "u-tone -mb-px border-b-2 px-3 py-2 text-body",
                tab === x.id ? "border-accent text-text" : "border-transparent text-text-muted hover:text-text",
              )}
            >
              {x.label}
            </button>
          ))}
        </div>
        {tab === "sources" ? (
          <div role="tabpanel" id={`${tabsId}-sources-panel`} aria-labelledby={`${tabsId}-sources`} className="flex flex-col gap-2">
            <p className="px-4 text-footnote text-text-muted">{t.sourcesHint}</p>
            <div className="rounded-card bg-surface">
              {song.sources.length === 0 ? (
                <p className="px-4 py-3 text-callout text-text-muted">{t.noSources}</p>
              ) : (
                <ul aria-label={t.sources} className="flex flex-col divide-y divide-border">
                  {song.sources.map((s) => (
                    <SourceRow
                      key={s.key}
                      source={s}
                      selected={s.key === sourceKey}
                      quality={qualityOf(s)}
                      onSelect={() => setSourceKey(s.key)}
                      onQuality={(q) => {
                        setQualities((prev) => ({ ...prev, [s.key]: q }));
                        setSourceKey(s.key);
                      }}
                    />
                  ))}
                </ul>
              )}
            </div>
          </div>
        ) : (
          <div role="tabpanel" id={`${tabsId}-details-panel`} aria-labelledby={`${tabsId}-details`} className="rounded-card bg-surface">
            <dl>
              <InfoRow label={t.artists}>{song.artists.join(", ") || t.unknown}</InfoRow>
              <InfoRow label={t.album}>{song.album ?? t.unknown}</InfoRow>
              <InfoRow label={t.length}>{song.durationMs ? formatDuration(song.durationMs) : t.unknown}</InfoRow>
              <InfoRow label={t.isrc}>{song.isrc ?? t.unknown}</InfoRow>
              <InfoRow label={t.sourceCount}>{song.sources.map((s) => s.extensionName).join(", ")}</InfoRow>
            </dl>
          </div>
        )}
      </div>
    </>
  );
}

function PageSkeleton() {
  return (
    <div className="flex items-end gap-6 pt-2" aria-hidden="true">
      <Skeleton className="size-44 rounded-card" />
      <div className="flex flex-1 flex-col gap-3">
        <Skeleton shape="text" className="h-7 w-64" />
        <Skeleton shape="text" className="w-40" />
        <Skeleton shape="text" className="h-8 w-32" />
      </div>
    </div>
  );
}

/** A song from Search: artwork and info, its sources with qualities, play and like. */
export function Track() {
  const { key = "" } = useParams();
  const navigate = useNavigate();
  const song = useSong(key);

  let body;
  if (song.data) {
    body = <SongPage detail={song.data} />;
  } else if (song.isError) {
    body =
      isAppError(song.error) && song.error.code.type === "notFound" ? (
        <EmptyState icon={Music} title={t.notFoundTitle} body={t.notFoundBody} />
      ) : (
        <EmptyState
          icon={Music}
          body={errorMessage(song.error)}
          action={
            <Button size="sm" variant="secondary" onClick={() => void song.refetch()}>
              {strings.common.retry}
            </Button>
          }
        />
      );
  } else {
    body = <PageSkeleton />;
  }

  return (
    <div className="flex min-h-full flex-col px-8 pt-4 pb-8">
      <div className="mx-auto flex w-full max-w-[880px] flex-col gap-8">
        <button
          type="button"
          onClick={() => void navigate(-1)}
          className="u-tone -mb-4 flex w-fit items-center gap-1 rounded-[6px] text-callout text-text-muted"
        >
          <Icon as={ChevronLeft} size={12} />
          {t.back}
        </button>
        {body}
      </div>
    </div>
  );
}
