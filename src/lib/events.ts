import { useQueryClient } from "@tanstack/react-query";
import { useEffect } from "react";
import { format, strings } from "@/i18n";
import { events } from "@/lib/bindings";
import { errorMessage } from "@/lib/errors";
import { isTauri } from "@/lib/window";
import { useSyncStore } from "@/stores/sync";
import { toast } from "@/stores/toast";

const intl = new Intl.NumberFormat();

/** Subscribes to Rust sync events: progress → store, finished → refetch + toast. Mount once. */
export function useSyncEvents() {
  const qc = useQueryClient();

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const unlisten: Array<() => void> = [];
    const keep = (p: Promise<() => void>) =>
      void p.then((fn) => (disposed ? fn() : unlisten.push(fn)));

    keep(
      events.syncProgress.listen(({ payload }) => {
        useSyncStore.getState().progress(payload.provider, {
          stage: payload.stage,
          done: payload.done,
          total: payload.total,
        });
        // First event of a run: refresh accounts so the row shows "Syncing".
        if (payload.stage === "profile") void qc.invalidateQueries({ queryKey: ["accounts"] });
      }),
    );
    keep(
      events.syncFinished.listen(({ payload }) => {
        useSyncStore.getState().finish(payload.provider);
        void qc.invalidateQueries();
        if (payload.error) {
          toast({ title: strings.sync.failed, description: errorMessage(payload.error), tone: "danger" });
        } else if (payload.added > 0 || payload.removed > 0) {
          toast({
            title: strings.sync.finished,
            description: format(strings.sync.finishedBody, {
              added: intl.format(payload.added),
              removed: intl.format(payload.removed),
            }),
          });
        }
      }),
    );

    return () => {
      disposed = true;
      unlisten.forEach((fn) => fn());
    };
  }, [qc]);
}
