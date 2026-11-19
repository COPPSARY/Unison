import type { ReactNode } from "react";
import { useSearchParams } from "react-router";
import { Button, Drawer, Skeleton } from "@/components/ui";
import { strings } from "@/i18n";
import type { Account_Serialize as Account, AccountStatus } from "@/lib/bindings";
import { cn } from "@/lib/cn";
import { errorMessage } from "@/lib/errors";
import { useAccounts, useSyncNow } from "@/lib/queries";
import { relativeTime } from "@/lib/time";
import { format } from "@/i18n";
import { SpotifyConnectDrawer } from "./SpotifyConnectDrawer";

const a = strings.accounts;

const STATUS: Record<AccountStatus, { label: string; dot: string }> = {
  connected: { label: a.statusConnected, dot: "bg-success" },
  notConnected: { label: a.statusNotConnected, dot: "bg-text-tertiary" },
  needsReconnect: { label: a.statusNeedsReconnect, dot: "bg-warning" },
  syncing: { label: a.statusSyncing, dot: "bg-accent" },
};

function StatusLabel({ status }: { status: AccountStatus }) {
  const s = STATUS[status];
  return (
    <span className="flex items-center gap-1.5 text-callout text-text-muted">
      <span aria-hidden="true" className={cn("size-1.5 rounded-full", s.dot)} />
      {s.label}
    </span>
  );
}

function ProviderRow({
  name,
  account,
  action,
}: {
  name: string;
  account: Account | undefined;
  action: ReactNode;
}) {
  return (
    <div className="flex min-h-14 items-center gap-4 px-4 py-2 not-last:border-b not-last:border-border">
      <div className="min-w-0 flex-1">
        <p className="text-body text-text">{name}</p>
        <div className="flex items-center gap-2">
          {account ? <StatusLabel status={account.status} /> : <Skeleton shape="text" className="w-20" />}
          {account?.displayName ? (
            <span className="truncate text-callout text-text-tertiary">· {account.displayName}</span>
          ) : null}
          {account && account.status !== "notConnected" ? (
            <span className="text-callout text-text-tertiary">
              ·{" "}
              {account.lastSyncedAt
                ? format(strings.sync.lastSynced, { when: relativeTime(account.lastSyncedAt) })
                : strings.sync.never}
            </span>
          ) : null}
        </div>
      </div>
      {action}
    </div>
  );
}

/** Settings → Accounts (T-11; sync/disconnect controls arrive in T-16). */
export function AccountsSection() {
  const accounts = useAccounts();
  const syncNow = useSyncNow();
  const [params, setParams] = useSearchParams();
  const open = params.get("connect") === "spotify";
  const setOpen = (next: boolean) => {
    const p = new URLSearchParams(params);
    if (next) p.set("connect", "spotify");
    else p.delete("connect");
    setParams(p, { replace: true });
  };

  const spotify = accounts.data?.find((x) => x.provider === "spotify");
  const youtube = accounts.data?.find((x) => x.provider === "youtube");
  const spotifyConnected = spotify?.status === "connected";

  return (
    <div className="flex flex-col gap-2">
      <h2 className="px-4 text-footnote font-bold text-text-tertiary">{a.section}</h2>
      <section className="rounded-card bg-surface">
        {accounts.isError ? (
          <p role="alert" className="px-4 py-3 text-body text-danger">
            {errorMessage(accounts.error)}
          </p>
        ) : (
          <>
            <ProviderRow
              name={strings.source.spotify}
              account={spotify}
              action={
                <div className="flex items-center gap-2">
                {spotifyConnected || spotify?.status === "syncing" ? (
                  <Button
                    variant="secondary"
                    size="sm"
                    disabled={spotify?.status === "syncing" || syncNow.isPending}
                    onClick={() => syncNow.mutate("spotify")}
                  >
                    {strings.sync.syncNow}
                  </Button>
                ) : null}
                <Drawer open={open} onOpenChange={setOpen}>
                  <Button
                    variant={spotifyConnected ? "secondary" : "primary"}
                    size="sm"
                    onClick={() => setOpen(true)}
                  >
                    {spotifyConnected || spotify?.status === "needsReconnect"
                      ? a.reconnect
                      : a.connect}
                  </Button>
                  {open ? <SpotifyConnectDrawer onDone={() => setOpen(false)} /> : null}
                </Drawer>
                </div>
              }
            />
            <ProviderRow
              name={strings.source.youtube}
              account={youtube}
              action={<span className="text-callout text-text-tertiary">{a.youtubeLater}</span>}
            />
          </>
        )}
      </section>
    </div>
  );
}
