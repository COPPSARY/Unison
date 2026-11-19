import { Check, Copy, ExternalLink } from "lucide-react";
import { useEffect, useState } from "react";
import { Button, DrawerContent, Icon, IconButton, Input } from "@/components/ui";
import { format, strings } from "@/i18n";
import { commands } from "@/lib/bindings";
import { errorMessage, isAppError } from "@/lib/errors";
import { useConnectSpotify, useSpotifyHasClientId, useSpotifyRedirectUris } from "@/lib/queries";
import { toast } from "@/stores/toast";

const a = strings.accounts;
const CLIENT_ID = /^[0-9a-f]{32}$/i;
/** After this long without a callback, show what usually went wrong. */
const HINT_AFTER_MS = 20_000;

function CopyRow({ value }: { value: string }) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const t = setTimeout(() => setCopied(false), 1500);
    return () => clearTimeout(t);
  }, [copied]);

  return (
    <div className="flex items-center gap-1 rounded-control bg-fill-tertiary py-0.5 pr-0.5 pl-2.5">
      <code className="flex-1 truncate font-mono text-callout text-text select-text">{value}</code>
      <IconButton
        size="sm"
        label={copied ? a.copied : `${a.copy} ${value}`}
        icon={<Icon as={copied ? Check : Copy} />}
        onClick={() => {
          void navigator.clipboard?.writeText(value).then(() => setCopied(true));
        }}
      />
    </div>
  );
}

/** BYOK onboarding (spec §3 Accounts & BYOK): dashboard · redirect URIs · Client ID → connect. */
export function SpotifyConnectDrawer({ onDone }: { onDone: () => void }) {
  const hasClientId = useSpotifyHasClientId();
  const uris = useSpotifyRedirectUris();
  const connect = useConnectSpotify();
  const [clientId, setClientId] = useState("");
  const [formatError, setFormatError] = useState<string | null>(null);
  const [showHint, setShowHint] = useState(false);

  const waiting = connect.isPending;

  useEffect(() => {
    if (!waiting) {
      setShowHint(false);
      return;
    }
    const t = setTimeout(() => setShowHint(true), HINT_AFTER_MS);
    return () => clearTimeout(t);
  }, [waiting]);

  const submit = () => {
    const trimmed = clientId.trim();
    if (trimmed && !CLIENT_ID.test(trimmed)) {
      setFormatError(strings.errors.invalidClientId);
      return;
    }
    if (!trimmed && !hasClientId.data) {
      setFormatError(a.clientIdRequired);
      return;
    }
    setFormatError(null);
    connect.mutate(trimmed || null, {
      onSuccess: (account) => {
        toast({
          title: a.connectedToast,
          ...(account.displayName
            ? { description: format(a.connectedAs, { name: account.displayName }) }
            : {}),
        });
        onDone();
      },
    });
  };

  // A cancel the user asked for isn't an error worth showing.
  const error =
    connect.isError && !(isAppError(connect.error) && connect.error.code.type === "authCancelled")
      ? errorMessage(connect.error)
      : null;

  return (
    <DrawerContent
      title={a.byokTitle}
      footer={
        waiting ? (
          <Button variant="secondary" onClick={() => void commands.accountCancelConnect()}>
            {a.cancel}
          </Button>
        ) : (
          <Button variant="primary" onClick={submit}>
            {a.connect}
          </Button>
        )
      }
    >
      <ol className="flex flex-col gap-6">
        <li className="flex flex-col gap-2">
          <p className="text-body text-text">
            <span className="text-text-tertiary tabular-nums">1 </span>
            {a.byokStep1}
          </p>
          <Button
            variant="secondary"
            size="sm"
            className="self-start"
            onClick={() => void commands.spotifyOpenDashboard()}
          >
            {a.openDashboard} <Icon as={ExternalLink} size={12} />
          </Button>
        </li>

        <li className="flex flex-col gap-2">
          <p className="text-body text-text">
            <span className="text-text-tertiary tabular-nums">2 </span>
            {a.byokStep2}
          </p>
          {(uris.data ?? []).map((uri) => (
            <CopyRow key={uri} value={uri} />
          ))}
          <p className="text-callout text-text-muted">{a.byokUsers}</p>
        </li>

        <li className="flex flex-col gap-2">
          <label htmlFor="spotify-client-id" className="text-body text-text">
            <span className="text-text-tertiary tabular-nums">3 </span>
            {a.byokStep3}
          </label>
          <Input
            id="spotify-client-id"
            value={clientId}
            onChange={(e) => {
              setClientId(e.target.value);
              setFormatError(null);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !waiting) submit();
            }}
            placeholder={a.clientId}
            autoComplete="off"
            spellCheck={false}
            disabled={waiting}
            invalid={formatError !== null}
            aria-describedby="spotify-client-id-help"
            className="font-mono"
          />
          <p
            id="spotify-client-id-help"
            role={formatError ? "alert" : undefined}
            className={formatError ? "text-callout text-danger" : "text-callout text-text-muted"}
          >
            {formatError ?? (hasClientId.data ? a.clientIdSaved : "")}
          </p>
        </li>
      </ol>

      {waiting ? (
        <div role="status" className="mt-6 rounded-card bg-fill-quaternary p-3">
          <p className="text-body text-text">{a.waiting}</p>
          {showHint ? <p className="mt-1.5 text-callout text-text-muted">{a.waitingHint}</p> : null}
        </div>
      ) : null}
      {error ? (
        <p role="alert" className="mt-6 rounded-card bg-fill-quaternary p-3 text-body text-danger">
          {error}
        </p>
      ) : null}
    </DrawerContent>
  );
}
