import { Blocks, Check, ChevronLeft, Minus } from "lucide-react";
import type { ReactNode } from "react";
import { Link, useParams } from "react-router";
import { EmptyState } from "@/components/shell/Page";
import { Button, Icon, Skeleton, Switch } from "@/components/ui";
import { format, strings } from "@/i18n";
import { installState, registryName, useCatalog, useExtensionActions, type CatalogItem } from "./catalog";
import { ExtensionIcon } from "./ExtensionIcon";
import { pinLine, typeInfo } from "./format";

const t = strings.extensions;
const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" });

function Block({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section aria-label={title} className="flex flex-col gap-2">
      <h2 className="px-4 text-footnote font-bold text-text-tertiary">{title}</h2>
      <div className="rounded-card bg-surface">{children}</div>
    </section>
  );
}

function InfoRow({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex min-h-11 items-start gap-4 px-4 py-2.5 not-last:border-b not-last:border-border">
      <dt className="w-32 shrink-0 text-body text-text-muted">{label}</dt>
      <dd className="min-w-0 flex-1 text-body break-words text-text">{children}</dd>
    </div>
  );
}

function Capabilities({ item }: { item: CatalogItem }) {
  const inUnison = [
    { on: item.capabilities.includes("search"), label: t.canSearch },
    { on: item.capabilities.includes("play"), label: t.canPlay },
  ];
  const known = item.preview !== null || item.installed !== null;
  return (
    <div className="flex flex-col">
      {item.types.length === 0 ? (
        <p className="px-4 py-3 text-callout text-text-muted">{t.noTypes}</p>
      ) : (
        <ul aria-label={t.capabilities} className="flex flex-col">
          {item.types.map((type) => {
            const info = typeInfo(type);
            return (
              <li key={type} className="flex flex-col gap-0.5 px-4 py-2.5 not-last:border-b not-last:border-border">
                <span className="text-body text-text">{info.title}</span>
                <span className="text-footnote text-text-muted">{info.body}</span>
              </li>
            );
          })}
        </ul>
      )}
      {known ? (
        <div className="flex flex-col gap-1.5 border-t border-border px-4 py-3">
          <span className="text-footnote font-bold text-text-tertiary">{t.inUnison}</span>
          {inUnison.some((c) => c.on) ? (
            <ul aria-label={t.inUnison} className="flex flex-col gap-1">
              {inUnison.map((c) => (
                <li
                  key={c.label}
                  className={c.on ? "flex items-center gap-2 text-callout text-text" : "flex items-center gap-2 text-callout text-text-tertiary"}
                >
                  <Icon as={c.on ? Check : Minus} size={12} className="text-text-muted" />
                  {c.label}
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-callout text-text-muted">{t.cannot}</p>
          )}
        </div>
      ) : null}
    </div>
  );
}

function Header({ item, actions }: { item: CatalogItem; actions: ReturnType<typeof useExtensionActions> }) {
  const state = installState(item);
  const busy = actions.busy !== null;
  const installing = actions.busy === item.key;
  const sub = [item.author ? format(t.by, { author: item.author }) : null, item.version ? format(t.version, { version: item.version }) : null]
    .filter(Boolean)
    .join(" · ");
  return (
    <div className="flex items-end gap-6 pt-2 pb-2">
      <ExtensionIcon src={item.icon} className="size-24 rounded-[20px]" />
      <div className="flex min-w-0 flex-1 flex-col gap-3">
        <div className="flex min-w-0 flex-col gap-1">
          <h1 className="truncate text-large-title text-text">{item.name || t.unnamed}</h1>
          {sub ? <p className="truncate text-callout text-text-muted tabular-nums">{sub}</p> : null}
        </div>
        <div className="flex flex-wrap items-center gap-2">
          {state === "new" || state === "update" ? (
            <Button
              variant="primary"
              disabled={busy}
              onClick={() => item.preview && void actions.install(item.key, item.preview)}
            >
              {installing ? t.installing : state === "update" ? t.update : t.install}
            </Button>
          ) : state === "unavailable" ? (
            <Button variant="secondary" disabled>
              {t.notInstallable}
            </Button>
          ) : null}
          {item.installed ? (
            <>
              <Switch
                checked={item.installed.enabled}
                disabled={item.installed.unsupportedFeature !== null}
                onCheckedChange={(on) => item.installed && void actions.setEnabled(item.key, item.installed.id, on)}
                label={format(t.enable, { name: item.name })}
              />
              <span className="text-callout text-text-muted">{item.installed.enabled ? t.enabled : t.off}</span>
              <Button
                variant="ghost"
                disabled={busy}
                onClick={() => item.installed && void actions.remove(item.key, item.installed.id)}
              >
                {format(t.remove, { name: item.name })}
              </Button>
            </>
          ) : null}
        </div>
        {item.unsupportedFeature ? (
          <p className="text-footnote text-text-muted">{format(t.unsupportedFeature, { feature: item.unsupportedFeature })}</p>
        ) : null}
        {actions.errors[item.key] ? (
          <p role="alert" className="text-footnote text-danger">
            {actions.errors[item.key]}
          </p>
        ) : null}
      </div>
    </div>
  );
}

function Information({ item }: { item: CatalogItem }) {
  const source = item.installed ?? item.preview;
  return (
    <dl>
      <InfoRow label={t.infoVersion}>{item.version ?? t.unknown}</InfoRow>
      <InfoRow label={t.infoAuthor}>{item.author ?? t.unknown}</InfoRow>
      {item.category ? <InfoRow label={t.infoCategory}>{item.category}</InfoRow> : null}
      {item.homepage ? <InfoRow label={t.infoHomepage}>{item.homepage}</InfoRow> : null}
      <InfoRow label={t.infoCode}>{item.runsCode === null ? t.unknown : item.runsCode ? t.yes : t.no}</InfoRow>
      <InfoRow label={t.infoHosts}>
        {item.hosts.length === 0 ? (
          t.infoNoHosts
        ) : (
          <ul aria-label={t.infoHosts} className="flex flex-col gap-0.5">
            {item.hosts.map((h) => (
              <li key={h}>{h}</li>
            ))}
          </ul>
        )}
      </InfoRow>
      {source ? <InfoRow label={t.infoSource}>{source.repoUrl}</InfoRow> : null}
      {source ? <InfoRow label={t.infoPinned}><span className="tabular-nums">{pinLine(source.commitSha)}</span></InfoRow> : null}
      {item.installed ? (
        <InfoRow label={t.infoInstalled}>{dateFormat.format(item.installed.installedAt * 1000)}</InfoRow>
      ) : null}
      {item.registry ? <InfoRow label={t.infoRegistry}>{registryName(item.registry)}</InfoRow> : null}
    </dl>
  );
}

function DetailSkeleton() {
  return (
    <div className="flex items-end gap-6 pt-2" aria-hidden="true">
      <Skeleton className="size-24 rounded-[20px]" />
      <div className="flex flex-1 flex-col gap-3">
        <Skeleton shape="text" className="h-6 w-56" />
        <Skeleton shape="text" className="w-32" />
        <Skeleton shape="text" className="h-8 w-40" />
      </div>
    </div>
  );
}

/** One extension: what it is, what it can do in Unison, and where it comes from. */
export function ExtensionDetail() {
  const { key = "" } = useParams();
  const catalog = useCatalog();
  const actions = useExtensionActions();
  const item = catalog.find(key);

  let body;
  if (item) {
    body = (
      <>
        <Header item={item} actions={actions} />
        <Block title={t.about}>
          <p className="px-4 py-3 text-body whitespace-pre-line text-text">{item.description ?? t.noDescription}</p>
        </Block>
        {item.tags.length > 0 || item.category ? (
          <Block title={t.tags}>
            <ul aria-label={t.tags} className="flex flex-wrap gap-1.5 px-4 py-3">
              {[...(item.category ? [item.category] : []), ...item.tags].map((tag) => (
                <li key={tag} className="rounded-full bg-fill-quaternary px-2.5 py-0.5 text-footnote text-text-muted">
                  {tag}
                </li>
              ))}
            </ul>
          </Block>
        ) : null}
        <Block title={t.capabilities}>
          <Capabilities item={item} />
        </Block>
        <Block title={t.information}>
          <Information item={item} />
        </Block>
      </>
    );
  } else if (catalog.loading) {
    body = <DetailSkeleton />;
  } else {
    body = <EmptyState icon={Blocks} title={t.notFoundTitle} body={t.notFoundBody} />;
  }

  return (
    <div className="flex min-h-full flex-col px-8 pt-4 pb-8">
      <div className="mx-auto flex w-full max-w-[880px] flex-col gap-8">
        <Link
          to="/extensions"
          className="u-tone -mb-4 flex w-fit items-center gap-1 rounded-[6px] text-callout text-text-muted"
        >
          <Icon as={ChevronLeft} size={12} />
          {t.backToList}
        </Link>
        {body}
      </div>
    </div>
  );
}
