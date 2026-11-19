import { Disc3 } from "lucide-react";
import { EmptyState, Page } from "@/components/shell/Page";
import { strings } from "@/i18n";

const t = strings.pages.album;

export function Album() {
  return (
    <Page title={t.title}>
      <EmptyState icon={Disc3} title={t.emptyTitle} body={t.emptyBody} />
    </Page>
  );
}
