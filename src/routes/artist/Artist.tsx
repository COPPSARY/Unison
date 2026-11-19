import { MicVocal } from "lucide-react";
import { EmptyState, Page } from "@/components/shell/Page";
import { strings } from "@/i18n";

const t = strings.pages.artist;

export function Artist() {
  return (
    <Page title={t.title}>
      <EmptyState icon={MicVocal} title={t.emptyTitle} body={t.emptyBody} />
    </Page>
  );
}
