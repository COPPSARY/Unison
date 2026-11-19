import { format, strings } from "@/i18n";
import type { Issue, LocalImportProgress_Deserialize as Progress, Track_Serialize as Track } from "@/lib/bindings";
import { commands } from "@/lib/bindings";
import { errorMessage } from "@/lib/errors";
import { useLocalImportStore } from "@/stores/localImport";
import { toast } from "@/stores/toast";

const t = strings.pages.local;
const r = strings.localImport;
const intl = new Intl.NumberFormat();

const fail = (err: unknown) => toast({ title: errorMessage(err), tone: "danger" });

/** Native "Add files" dialog; the import then reports through events. */
export function addFiles() {
  commands.localAddFiles(t.filesDialogTitle, t.audioFilter).catch(fail);
}

/** Native "Add folder" dialog (the folder is referenced in place). */
export function addFolder() {
  commands.localAddFolder(t.folderDialogTitle).catch(fail);
}

/** The song's local file was moved or deleted (it can be located). */
export function missingFile(track: Track): boolean {
  return track.sources.some((s) => s.provider === "local" && !s.available);
}

/** "Locate file…": pick the moved file; the song plays from it again. Resolves true when found. */
export async function locateFile(track: Track): Promise<boolean> {
  try {
    const path = await commands.localPickFile(format(t.locateTitle, { title: track.title }), t.audioFilter);
    if (path === null) return false;
    await commands.localLocate(track.id, path);
    toast({ title: format(t.located, { title: track.title }) });
    return true;
  } catch (err) {
    fail(err);
    return false;
  }
}

export function reasonLabel(reason: Issue["reason"]): string {
  return r.reason[reason];
}

/** Handles a `local://import-progress` event: progress → store, finish → one summary toast. */
export function onImportProgress(p: Progress) {
  const store = useLocalImportStore.getState();
  if (!p.finished) {
    store.progress({ jobId: p.jobId, scanned: p.scanned, total: p.total ?? null, imported: p.imported });
    return;
  }
  store.finish(p.errors);
  if (p.error) {
    toast({ title: r.failed, description: errorMessage(p.error), tone: "danger" });
    return;
  }
  const issues = p.errors.length;
  const issuesText = issues === 1 ? r.issuesOne : format(r.issues, { count: intl.format(issues) });
  const removedText = p.removed === 1 ? r.removedOne : format(r.removed, { count: intl.format(p.removed) });
  const details = issues > 0 ? { action: { label: r.details, onAction: store.openIssues } } : {};
  if (p.imported > 0) {
    const description = p.removed > 0 ? removedText : issues > 0 ? issuesText : null;
    toast({
      title: p.imported === 1 ? r.addedOne : format(r.added, { count: intl.format(p.imported) }),
      ...(description ? { description } : {}),
      ...details,
    });
  } else if (p.removed > 0) {
    toast({ title: removedText, description: r.removedBody });
  } else if (p.rescan && issues === 0) {
    toast({ title: r.upToDate });
  } else if (issues > 0) {
    toast({ title: issuesText, tone: "danger", ...details });
  } else if (p.skipped > 0) {
    toast({ title: r.nothingNew, description: r.nothingNewBody });
  } else {
    toast({ title: r.nothingFound });
  }
}
