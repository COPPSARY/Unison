import { useQueryClient } from "@tanstack/react-query";
import { Laptop, RefreshCw, Smartphone, Speaker } from "lucide-react";
import { useState } from "react";
import { Icon, IconButton, Skeleton } from "@/components/ui";
import { strings } from "@/i18n";
import { commands } from "@/lib/bindings";
import { cn } from "@/lib/cn";
import { errorMessage } from "@/lib/errors";
import { useAccounts, useSpotifyCapabilities } from "@/lib/queries";
import { spotifyStatus } from "./spotifyStatus";

const t = strings.settings.spotifyPlayback;

function DeviceIcon({ kind }: { kind: string }) {
  const k = kind.toLowerCase();
  return <Icon as={k === "smartphone" ? Smartphone : k === "computer" ? Laptop : Speaker} />;
}

/** Settings → Playback → Spotify (UP-04): Premium + Spotify Connect devices. */
export function SpotifyPlaybackRow() {
  const accounts = useAccounts();
  const connected = accounts.data?.some((a) => a.provider === "spotify" && a.status !== "notConnected") ?? false;
  const caps = useSpotifyCapabilities(connected);
  const qc = useQueryClient();
  const [refreshing, setRefreshing] = useState(false);

  const refresh = () => {
    setRefreshing(true);
    commands
      .spotifyCapabilities(true)
      .then((fresh) => qc.setQueryData(["spotifyCapabilities"], fresh))
      .catch(() => void caps.refetch())
      .finally(() => setRefreshing(false));
  };

  const status = spotifyStatus(caps.data, connected);

  return (
    <div className="flex flex-col gap-2 px-4 py-3">
      <div className="flex items-center justify-between gap-4">
        <div className="min-w-0">
          <p className="text-body text-text">{t.title}</p>
          {caps.isError ? (
            <p role="alert" className="text-callout text-danger">
              {errorMessage(caps.error)}
            </p>
          ) : caps.isPending && connected ? (
            <Skeleton shape="text" className="mt-1 w-32" />
          ) : (
            <p className="flex items-center gap-1.5 text-callout text-text-muted">
              <span aria-hidden="true" className={cn("size-1.5 rounded-full", status.dot)} />
              {status.label}
            </p>
          )}
        </div>
        {connected ? (
          <IconButton
            label={t.refresh}
            size="sm"
            disabled={refreshing}
            icon={<Icon as={RefreshCw} />}
            onClick={refresh}
          />
        ) : null}
      </div>
      {caps.data && caps.data.devices.length > 0 ? (
        <ul aria-label={t.devices} className="flex flex-col gap-1">
          {caps.data.devices.map((d) => (
            <li key={d.id ?? d.name} className="flex items-center gap-2 text-callout text-text-muted">
              <DeviceIcon kind={d.kind} />
              <span className={cn("truncate", d.isActive && "text-text")}>{d.name}</span>
              {d.isActive ? <span className="text-text-tertiary">· {t.active}</span> : null}
              {d.isRestricted ? <span className="text-text-tertiary">· {t.restricted}</span> : null}
            </li>
          ))}
        </ul>
      ) : null}
    </div>
  );
}
