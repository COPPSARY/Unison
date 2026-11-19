import { Settings as SettingsIcon } from "lucide-react";
import { EmptyState, Page } from "@/components/shell/Page";
import { strings } from "@/i18n";

const t = strings.pages.settings;

/** Settings (max 720 px). Sections arrive with T-11 / T-16 / T-71. */
export function Settings() {
  return (
    <Page title={t.title}>
      <div className="mx-auto flex w-full max-w-[720px] flex-1">
        <EmptyState icon={SettingsIcon} body={t.emptyBody} />
      </div>
    </Page>
  );
}
