import { useQuery, useQueryClient } from "@tanstack/react-query";
import { FolderPlus, RefreshCw, X } from "lucide-react";
import { useState } from "react";
import { Button, Icon, IconButton, Skeleton, Switch } from "@/components/ui";
import { format, strings } from "@/i18n";
import type { WatchedFolder_Serialize as WatchedFolder } from "@/lib/bindings";
import { commands } from "@/lib/bindings";
import { errorMessage } from "@/lib/errors";
import { formatDateTime } from "@/lib/format";
import { useWatchedFolders } from "@/lib/queries";
import { isTauri } from "@/lib/window";
import { useLocalImportStore } from "@/stores/localImport";
import { useUiStore } from "@/stores/ui";

const t = strings.settings.local;

function ErrorLine({ error }: { error: string | null }) {
  return error ? (
    <p role="alert" className="text-footnote text-danger">
      {error}
    </p>
  ) : null;
}

function FolderRow({ folder, onRemove }: { folder: WatchedFolder; onRemove: () => void }) {
  return (
    <li className="flex items-center gap-3 px-4 py-2">
      <div className="min-w-0 flex-1">
        <p className="truncate text-body text-text" title={folder.path}>
          {folder.path}
        </p>
        <p className={folder.exists ? "text-footnote text-text-muted" : "text-footnote text-danger"}>
          {!folder.exists
            ? t.folderMissing
            : folder.lastScanAt
              ? format(t.scanned, { when: formatDateTime(folder.lastScanAt) })
              : t.notScanned}
        </p>
      </div>
      <IconButton
        label={format(t.removeFolder, { path: folder.path })}
        icon={<Icon as={X} />}
        size="sm"
        onClick={onRemove}
      />
    </li>
  );
}

/** Watched folders (UP-12): list, Add folder…, Remove, Rescan library with last-scan time. */
function WatchedFolders() {
  const qc = useQueryClient();
  const folders = useWatchedFolders();
  const job = useLocalImportStore((s) => s.job);
  const [error, setError] = useState<string | null>(null);
  const [rescanning, setRescanning] = useState(false);
  const list = folders.data ?? [];
  const lastScan = Math.max(0, ...list.map((f) => f.lastScanAt ?? 0));

  const update = (p: Promise<WatchedFolder[]>) => {
    setError(null);
    p.then((next) => qc.setQueryData(["local", "folders"], next)).catch((err: unknown) => setError(errorMessage(err)));
  };
  const add = () => {
    setError(null);
    commands
      .localPickFolder(t.folderDialogTitle)
      .then((path) => {
        if (path !== null) update(commands.localFolderAdd(path));
      })
      .catch((err: unknown) => setError(errorMessage(err)));
  };
  const rescan = () => {
    setError(null);
    setRescanning(true);
    commands
      .localRescan()
      .catch((err: unknown) => setError(errorMessage(err)))
      .finally(() => setRescanning(false));
  };
  const busy = rescanning || job !== null;

  return (
    <section className="rounded-card bg-surface" aria-label={t.foldersTitle}>
      <div className="flex flex-col gap-1 px-4 pt-3 pb-2">
        <span className="text-body text-text">{t.foldersTitle}</span>
        <p className="text-footnote text-text-muted">{t.foldersHint}</p>
      </div>
      {folders.isError ? (
        <p role="alert" className="px-4 py-2 text-footnote text-danger">
          {t.loadError}
        </p>
      ) : !folders.data && isTauri() ? (
        <div className="px-4 py-2">
          <Skeleton shape="text" className="w-3/5" />
        </div>
      ) : list.length === 0 ? (
        <p className="px-4 py-2 text-callout text-text-tertiary">{t.noFolders}</p>
      ) : (
        <ul aria-label={t.foldersTitle}>
          {list.map((folder) => (
            <FolderRow
              key={folder.path}
              folder={folder}
              onRemove={() => update(commands.localFolderRemove(folder.path))}
            />
          ))}
        </ul>
      )}
      <div className="flex flex-wrap items-center gap-2 border-t border-border px-4 py-3">
        <Button size="sm" variant="secondary" onClick={add}>
          <Icon as={FolderPlus} /> {t.addFolder}
        </Button>
        <Button size="sm" variant="secondary" onClick={rescan} disabled={busy}>
          <Icon as={RefreshCw} /> {busy ? t.rescanning : t.rescan}
        </Button>
        {lastScan > 0 ? (
          <span className="text-footnote text-text-muted">
            {format(t.lastScan, { when: formatDateTime(lastScan) })}
          </span>
        ) : null}
      </div>
      <div className="px-4 pb-2">
        <ErrorLine error={error} />
      </div>
    </section>
  );
}

/** Settings → Local files: watched folders (UP-12) and copy-on-drop (UP-11). File types: UP-14. */
export function LocalSection() {
  const settings = useUiStore((s) => s.settings);
  const hydrate = useUiStore((s) => s.hydrate);
  const [error, setError] = useState<string | null>(null);
  const copy = settings?.local.copyOnDrop ?? false;
  const target = useQuery({
    queryKey: ["localCopyTarget"],
    queryFn: () => commands.localCopyTarget(),
    enabled: isTauri(),
  });

  const toggle = (next: boolean) => {
    if (!settings) return;
    setError(null);
    commands
      .settingsSet({ ...settings, local: { ...settings.local, copyOnDrop: next } })
      .then(hydrate)
      .catch((err: unknown) => setError(errorMessage(err)));
  };

  return (
    <div className="flex flex-col gap-2">
      <h2 className="px-4 text-footnote font-bold text-text-tertiary">{t.title}</h2>
      <WatchedFolders />
      <section className="rounded-card bg-surface">
        <div className="flex flex-col gap-1 px-4 py-3">
          <div className="flex items-center justify-between gap-4">
            <span className="text-body text-text">{t.copyOnDrop}</span>
            <Switch checked={copy} onCheckedChange={toggle} label={t.copyOnDrop} disabled={!settings} />
          </div>
          <p className="text-footnote text-text-muted">
            {copy && target.data ? format(t.copyTarget, { path: target.data }) : t.referenceHint}
          </p>
          <ErrorLine error={error} />
        </div>
      </section>
    </div>
  );
}
