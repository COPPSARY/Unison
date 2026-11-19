import type { ReactNode } from "react";
import { Page } from "@/components/shell/Page";
import { Skeleton } from "@/components/ui";
import { strings } from "@/i18n";
import { errorMessage } from "@/lib/errors";
import { useAppInfo } from "@/lib/queries";

const t = strings.pages.settings;
const s = strings.settings;

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex h-11 items-center justify-between px-4 not-last:border-b not-last:border-border">
      <span className="text-body text-text">{label}</span>
      <div className="text-body text-text-muted tabular-nums">{children}</div>
    </div>
  );
}

/** Settings (max 720 px). Accounts, playback, appearance, sync and data arrive with T-11/16/71. */
export function Settings() {
  const info = useAppInfo();

  return (
    <Page title={t.title}>
      <div className="mx-auto flex w-full max-w-[720px] flex-col gap-2">
        <h2 className="px-4 text-footnote font-bold text-text-tertiary">{s.about}</h2>
        <section className="rounded-card bg-surface">
          {info.isError ? (
            <p role="alert" className="px-4 py-3 text-body text-danger">
              {errorMessage(info.error)}
            </p>
          ) : (
            <>
              <Row label={s.version}>
                {info.data ? info.data.version : <Skeleton shape="text" className="w-12" />}
              </Row>
              <Row label={s.schema}>
                {info.data ? info.data.schemaVersion : <Skeleton shape="text" className="w-6" />}
              </Row>
            </>
          )}
        </section>
      </div>
    </Page>
  );
}
