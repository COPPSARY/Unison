import { Heart } from "lucide-react";
import { EmptyState, Page } from "@/components/shell/Page";
import { strings } from "@/i18n";

const t = strings.pages.liked;

export function Liked() {
  return (
    <Page title={t.title}>
      <EmptyState icon={Heart} title={t.emptyTitle} body={t.emptyBody} />
    </Page>
  );
}
