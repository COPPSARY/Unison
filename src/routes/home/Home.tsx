import { Blocks } from "lucide-react";
import { useNavigate } from "react-router";
import { EmptyState, Page } from "@/components/shell/Page";
import { Button } from "@/components/ui";
import { strings } from "@/i18n";

const t = strings.pages.home;

/** First-run Home: music comes from extensions. Real rows arrive in T-53. */
export function Home() {
  const navigate = useNavigate();
  return (
    <Page title={t.title}>
      <EmptyState
        icon={Blocks}
        title={t.emptyTitle}
        body={t.emptyBody}
        action={
          <Button variant="primary" onClick={() => void navigate("/extensions")}>
            {strings.nav.extensions}
          </Button>
        }
      />
    </Page>
  );
}
