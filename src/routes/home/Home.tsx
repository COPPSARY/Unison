import { Sparkles } from "lucide-react";
import { useNavigate } from "react-router";
import { EmptyState, Page } from "@/components/shell/Page";
import { Button } from "@/components/ui";
import { strings } from "@/i18n";

const t = strings.pages.home;

/** First-run Home: nothing connected yet (spec §3 Home states). Real rows arrive in T-53. */
export function Home() {
  const navigate = useNavigate();
  // Connecting happens on Settings → Accounts; YouTube arrives with T-20.
  const toSettings = () => void navigate("/settings");
  const connectSpotify = () => void navigate("/settings?connect=spotify");

  return (
    <Page title={t.title}>
      <EmptyState
        icon={Sparkles}
        title={t.emptyTitle}
        body={t.emptyBody}
        action={
          <div className="flex gap-2">
            <Button variant="primary" onClick={connectSpotify}>
              {strings.accounts.connectSpotify}
            </Button>
            <Button variant="secondary" onClick={toSettings}>
              {strings.accounts.connectYoutube}
            </Button>
          </div>
        }
      />
    </Page>
  );
}
