import { ListMusic } from "lucide-react";
import { EmptyState, Page } from "@/components/shell/Page";
import { strings } from "@/i18n";

const t = strings.pages.playlist;

export function Playlist() {
  return (
    <Page title={t.title}>
      <EmptyState icon={ListMusic} title={t.emptyTitle} body={t.emptyBody} />
    </Page>
  );
}
