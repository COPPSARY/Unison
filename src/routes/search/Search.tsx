import { Blocks, Play, Search as SearchIcon } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useNavigate } from "react-router";
import { EmptyState, Page } from "@/components/shell/Page";
import { Artwork } from "@/components/track/Artwork";
import { Button, Icon, Input, Skeleton } from "@/components/ui";
import { format, strings } from "@/i18n";
import type { SearchFailure, Song_Serialize as Song } from "@/lib/bindings";
import { cn } from "@/lib/cn";
import { errorMessage, messageForCode } from "@/lib/errors";
import { formatDuration } from "@/lib/format";
import { useExtensions, useSearchAll } from "@/lib/queries";
import { playSong, songHref } from "@/lib/songs";

const t = strings.pages.search;
const DEBOUNCE_MS = 300;

function useDebounced(value: string): string {
  const [debounced, setDebounced] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setDebounced(value), DEBOUNCE_MS);
    return () => clearTimeout(id);
  }, [value]);
  return debounced;
}

/** A result: the play button on the artwork plays it; the rest opens the song's page. */
function SongRow({ song }: { song: Song }) {
  const count = song.sources.length;
  return (
    <li className="group grid grid-cols-[40px_minmax(0,1fr)] items-center gap-3 rounded-[8px] px-3 py-2 hover:bg-fill-quaternary">
      <button
        type="button"
        aria-label={format(t.playSong, { title: song.title })}
        onClick={() => void playSong(song)}
        className="relative size-10 cursor-pointer rounded-[6px] outline-none focus-visible:ring-2 focus-visible:ring-accent"
      >
        <Artwork src={song.artwork} className="size-10" />
        <span className="absolute inset-0 flex items-center justify-center opacity-0 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100">
          <span className="flex size-7 items-center justify-center rounded-full bg-accent text-accent-foreground shadow-glass">
            <Icon as={Play} size={12} fill="currentColor" />
          </span>
        </span>
      </button>
      <Link
        to={songHref(song.key)}
        aria-label={format(t.openSong, { title: song.title })}
        className="grid min-w-0 grid-cols-[minmax(0,2fr)_minmax(0,1fr)_auto_56px] items-center gap-3 rounded-[6px] outline-none focus-visible:bg-fill-tertiary"
      >
        <span className="min-w-0">
          <span className="block truncate text-body text-text">{song.title}</span>
          <span className="block truncate text-callout text-text-muted">{song.artists.join(", ")}</span>
        </span>
        <span className="truncate text-callout text-text-muted">{song.album ?? ""}</span>
        <span className="text-footnote text-text-tertiary tabular-nums">
          {count === 1 ? t.sourceCountOne : format(t.sourceCount, { count })}
        </span>
        <span className="text-right text-callout text-text-muted tabular-nums">{formatDuration(song.durationMs)}</span>
      </Link>
    </li>
  );
}

function ResultsSkeleton() {
  return (
    <ul aria-hidden="true">
      {[0, 1, 2, 3, 4].map((i) => (
        <li key={i} className="flex items-center gap-3 px-3 py-2">
          <Skeleton className="size-10" />
          <div className="flex flex-1 flex-col gap-1.5">
            <Skeleton shape="text" className="w-1/3" />
            <Skeleton shape="text" className="w-1/5" />
          </div>
        </li>
      ))}
    </ul>
  );
}

function Failures({ failures, onRetry }: { failures: SearchFailure[]; onRetry: () => void }) {
  if (failures.length === 0) return null;
  return (
    <div className="flex items-start gap-3 px-3 py-2">
      <ul className="min-w-0 flex-1">
        {failures.map((f) => (
          <li key={f.extensionId} role="alert" className="text-footnote text-text-muted">
            {format(t.failed, { name: f.extensionName, reason: messageForCode(f.error.code) })}
          </li>
        ))}
      </ul>
      <Button size="sm" variant="secondary" onClick={onRetry}>
        {t.retry}
      </Button>
    </div>
  );
}

/** Search: one list of songs from every extension, like a music app. A click opens the song. */
export function Search() {
  const navigate = useNavigate();
  const [input, setInput] = useState("");
  const query = useDebounced(input).trim();
  const extensions = useExtensions();
  const enabledCount = (extensions.data ?? []).filter((e) => e.enabled).length;
  const results = useSearchAll(enabledCount > 0 ? query : "");

  let body;
  if (extensions.data && enabledCount === 0) {
    body = (
      <EmptyState
        icon={Blocks}
        body={t.noExtensions}
        action={
          <Button variant="secondary" onClick={() => void navigate("/extensions")}>
            {t.noExtensionsAction}
          </Button>
        }
      />
    );
  } else if (query === "") {
    body = <EmptyState icon={SearchIcon} body={t.emptyBody} />;
  } else if (results.isError) {
    body = (
      <EmptyState
        icon={SearchIcon}
        body={errorMessage(results.error)}
        action={
          <Button size="sm" variant="secondary" onClick={() => void results.refetch()}>
            {t.retry}
          </Button>
        }
      />
    );
  } else if (!results.data) {
    body = (
      <div aria-busy="true" aria-label={t.searching}>
        <ResultsSkeleton />
      </div>
    );
  } else if (results.data.songs.length === 0) {
    body = (
      <>
        <EmptyState icon={SearchIcon} body={format(t.noResults, { query })} />
        <Failures failures={results.data.failures} onRetry={() => void results.refetch()} />
      </>
    );
  } else {
    body = (
      <section aria-label={t.songs} className={cn("flex flex-col gap-1", results.isFetching && "opacity-70")}>
        <h2 className="px-3 text-headline text-text">{t.songs}</h2>
        <ul aria-label={t.songs}>
          {results.data.songs.map((song) => (
            <SongRow key={song.key} song={song} />
          ))}
        </ul>
        <Failures failures={results.data.failures} onRetry={() => void results.refetch()} />
      </section>
    );
  }

  return (
    <Page title={t.title}>
      <div className="flex flex-col gap-6">
        <div className="max-w-xl">
          <Input
            autoFocus
            shape="capsule"
            leading={<Icon as={SearchIcon} />}
            placeholder={t.placeholder}
            aria-label={t.title}
            className="h-9"
            value={input}
            onChange={(e) => setInput(e.target.value)}
          />
        </div>
        {body}
      </div>
    </Page>
  );
}
