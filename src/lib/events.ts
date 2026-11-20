import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { strings } from "@/i18n";
import { events } from "@/lib/bindings";
import { messageForCode } from "@/lib/errors";
import { onImportProgress } from "@/lib/local";
import { isTauri } from "@/lib/window";
import { useDownloadsStore } from "@/stores/downloads";
import { useLocalImportStore } from "@/stores/localImport";
import { toast } from "@/stores/toast";

/** Subscribes to local-file events: drag overlay, import progress / summary, library refresh. */
export function useLocalEvents() {
  const qc = useQueryClient();

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const unlisten: Array<() => void> = [];
    const keep = (p: Promise<() => void>) =>
      void p.then((fn) => (disposed ? fn() : unlisten.push(fn)));

    keep(events.localDrag.listen(({ payload }) => useLocalImportStore.getState().setDragging(payload.active)));
    keep(
      events.localImportProgress.listen(({ payload }) => {
        onImportProgress(payload);
        // New rows land batch by batch: refresh the local list as they arrive.
        // The last event also updates watched folders' scan times.
        if (payload.imported > 0 || payload.finished) void qc.invalidateQueries({ queryKey: ["local"] });
      }),
    );
    keep(events.localChanged.listen(() => void qc.invalidateQueries()));
    keep(
      events.downloadProgress.listen(({ payload }) => {
        const store = useDownloadsStore.getState();
        if (!payload.finished) {
          store.progress(payload.jobId, payload.received ?? 0, payload.total ?? null);
          return;
        }
        store.finished(payload.jobId);
        const t = strings.pages.track;
        if (payload.error) {
          toast({ title: t.downloadFailed, description: messageForCode(payload.error.code), tone: "danger" });
        } else {
          toast({ title: t.downloaded, ...(payload.path ? { description: payload.path } : {}) });
          void qc.invalidateQueries({ queryKey: ["song"] });
        }
      }),
    );

    return () => {
      disposed = true;
      unlisten.forEach((fn) => fn());
    };
  }, [qc]);
}
