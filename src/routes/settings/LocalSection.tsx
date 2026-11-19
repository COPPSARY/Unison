import { useQuery } from "@tanstack/react-query";
import { useState } from "react";
import { Switch } from "@/components/ui";
import { format, strings } from "@/i18n";
import { commands } from "@/lib/bindings";
import { errorMessage } from "@/lib/errors";
import { isTauri } from "@/lib/window";
import { useUiStore } from "@/stores/ui";

const t = strings.settings.local;

/** Settings → Local files: copy-on-drop (UP-11). Folders and file types arrive with UP-12 / UP-14. */
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
      <section className="rounded-card bg-surface">
        <div className="flex flex-col gap-1 px-4 py-3">
          <div className="flex items-center justify-between gap-4">
            <span className="text-body text-text">{t.copyOnDrop}</span>
            <Switch checked={copy} onCheckedChange={toggle} label={t.copyOnDrop} disabled={!settings} />
          </div>
          <p className="text-footnote text-text-muted">
            {copy && target.data ? format(t.copyTarget, { path: target.data }) : t.referenceHint}
          </p>
          {error ? (
            <p role="alert" className="text-footnote text-danger">
              {error}
            </p>
          ) : null}
        </div>
      </section>
    </div>
  );
}
