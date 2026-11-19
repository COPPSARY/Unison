import { Search as SearchIcon } from "lucide-react";
import { EmptyState, Page } from "@/components/shell/Page";
import { Icon, Input } from "@/components/ui";
import { strings } from "@/i18n";

const t = strings.pages.search;

/** Search page. Ctrl+K lands here and focuses the field; the palette overlay is T-32. */
export function Search() {
  return (
    <Page title={t.title}>
      <div className="max-w-xl">
        <Input
          autoFocus
          shape="capsule"
          leading={<Icon as={SearchIcon} />}
          placeholder={t.placeholder}
          aria-label={t.title}
          className="h-9"
        />
      </div>
      <EmptyState icon={SearchIcon} body={t.emptyBody} />
    </Page>
  );
}
