import { useQueryClient } from "@tanstack/react-query";
import { Blocks, Link2, X } from "lucide-react";
import { useState, type ClipboardEvent, type FormEvent } from "react";
import { Link } from "react-router";
import { EmptyState, Page } from "@/components/shell/Page";
import { Button, Dialog, DialogClose, DialogContent, Icon, IconButton, Input, Skeleton } from "@/components/ui";
import { format, strings } from "@/i18n";
import { commands, type ExtensionPreview_Serialize as Preview } from "@/lib/bindings";
import { errorMessage, messageForCode } from "@/lib/errors";
import { queryKeys } from "@/lib/queries";
import {
  installState,
  isRegistryLink,
  registryName,
  useCatalog,
  useExtensionActions,
  type CatalogItem,
  type RegistrySection,
} from "./catalog";
import { ExtensionIcon } from "./ExtensionIcon";
import { addsLine, detailsHref, pinLine } from "./format";

const t = strings.extensions;

function ErrorLine({ error }: { error: string | null | undefined }) {
  return error ? (
    <p role="alert" className="text-footnote text-danger">
      {error}
    </p>
  ) : null;
}

/** One extension: icon, name, version, and its Install / Update button. The text opens details. */
function ExtensionRow({
  item,
  busy,
  error,
  onInstall,
}: {
  item: CatalogItem;
  busy: string | null;
  error: string | undefined;
  onInstall: (p: Preview) => void;
}) {
  const state = installState(item);
  const installing = busy === item.key;
  const meta = [
    state === "update" && item.installed && item.preview
      ? format(t.updateFrom, { from: item.installed.version, to: item.preview.version })
      : item.version
        ? format(t.version, { version: item.version })
        : null,
    item.author ? format(t.by, { author: item.author }) : null,
    item.installed && !item.installed.enabled ? t.off : null,
  ]
    .filter(Boolean)
    .join(" · ");
  const label =
    installing ? t.installing : state === "installed" ? t.installedAlready : state === "update" ? t.update : t.install;
  return (
    <li className="flex items-center gap-3 px-4 py-2">
      <Link
        to={detailsHref(item.key)}
        aria-label={format(t.openDetails, { name: item.name })}
        className="u-tone flex min-w-0 flex-1 items-center gap-3 rounded-[8px]"
      >
        <ExtensionIcon src={item.icon} className="size-10" />
        <span className="min-w-0 flex-1">
          <span className="block truncate text-body text-text">{item.name || t.unnamed}</span>
          <span className="block truncate text-footnote text-text-muted tabular-nums">{meta}</span>
          {item.error && !item.preview ? (
            <span className="block truncate text-footnote text-text-tertiary">{messageForCode(item.error.code)}</span>
          ) : null}
          {item.unsupportedFeature ? (
            <span className="block truncate text-footnote text-text-tertiary">
              {format(t.unsupportedFeature, { feature: item.unsupportedFeature })}
            </span>
          ) : null}
          <ErrorLine error={error} />
        </span>
      </Link>
      {state === "unavailable" ? (
        <Button size="sm" variant="secondary" disabled aria-label={format(t.installName, { name: item.name })}>
          {t.notInstallable}
        </Button>
      ) : (
        <Button
          size="sm"
          variant={state === "installed" ? "secondary" : "primary"}
          disabled={state === "installed" || busy !== null}
          onClick={() => item.preview && onInstall(item.preview)}
          aria-label={format(state === "update" ? t.updateName : t.installName, { name: item.name })}
        >
          {label}
        </Button>
      )}
    </li>
  );
}

function RowsSkeleton() {
  return (
    <div className="flex flex-col gap-3 px-4 py-3" aria-hidden="true">
      {[0, 1, 2].map((i) => (
        <div key={i} className="flex items-center gap-3">
          <Skeleton className="size-10 rounded-[10px]" />
          <div className="flex flex-1 flex-col gap-1.5">
            <Skeleton shape="text" className="w-1/3" />
            <Skeleton shape="text" className="w-1/5" />
          </div>
        </div>
      ))}
    </div>
  );
}

function Section({ title, action, children }: { title: string; action?: React.ReactNode; children: React.ReactNode }) {
  return (
    <section aria-label={title} className="flex flex-col gap-2">
      <div className="flex h-7 items-center justify-between px-4">
        <h2 className="truncate text-footnote font-bold text-text-tertiary">{title}</h2>
        {action}
      </div>
      <div className="rounded-card bg-surface">{children}</div>
    </section>
  );
}

function RegistryBlock({
  section,
  busy,
  errors,
  onInstall,
  onRemove,
}: {
  section: RegistrySection;
  busy: string | null;
  errors: Readonly<Record<string, string>>;
  onInstall: (key: string, p: Preview) => void;
  onRemove: () => void;
}) {
  const name = registryName(section.url);
  let body;
  if (section.error) {
    body = (
      <div className="flex items-center gap-3 px-4 py-3">
        <p role="alert" className="min-w-0 flex-1 text-footnote text-danger">
          {t.registryError}: {errorMessage(section.error)}
        </p>
        <Button size="sm" variant="secondary" onClick={section.refetch}>
          {strings.common.retry}
        </Button>
      </div>
    );
  } else if (section.loading) {
    body = <RowsSkeleton />;
  } else {
    body = (
      <ul aria-label={format(t.registryTitle, { name })} className="flex flex-col divide-y divide-border py-1">
        {section.items.map((item, i) => (
          <ExtensionRow
            key={`${i}:${item.key}`}
            item={item}
            busy={busy}
            error={errors[item.key]}
            onInstall={(p) => onInstall(item.key, p)}
          />
        ))}
      </ul>
    );
  }
  return (
    <Section
      title={format(t.registryTitle, { name })}
      action={
        <IconButton
          label={format(t.removeRegistry, { name })}
          icon={<Icon as={X} />}
          size="sm"
          onClick={onRemove}
        />
      }
    >
      {body}
    </Section>
  );
}

/** Confirms installing one extension from a pasted link. */
function PreviewDialog({
  preview,
  installing,
  error,
  onInstall,
  onClose,
}: {
  preview: Preview | null;
  installing: boolean;
  error: string | null | undefined;
  onInstall: () => void;
  onClose: () => void;
}) {
  return (
    <Dialog open={preview !== null} onOpenChange={(next) => (next ? undefined : onClose())}>
      {preview ? (
        <DialogContent
          title={format(t.previewTitle, { name: preview.name })}
          {...(preview.description ? { description: preview.description } : {})}
          footer={
            <>
              <DialogClose asChild>
                <Button variant="secondary" disabled={installing}>
                  {t.cancel}
                </Button>
              </DialogClose>
              <Button variant="primary" onClick={onInstall} disabled={installing || preview.unsupportedFeature !== null}>
                {installing ? t.installing : t.install}
              </Button>
            </>
          }
        >
          <div className="flex flex-col gap-3">
            <div className="flex items-center gap-3">
              <ExtensionIcon src={preview.icon} className="size-12" />
              <div className="flex min-w-0 flex-col gap-0.5 text-footnote text-text-muted">
                {preview.author ? <span>{format(t.by, { author: preview.author })}</span> : null}
                <span>
                  {preview.installedVersion !== null
                    ? format(t.replaces, { version: preview.installedVersion })
                    : format(t.version, { version: preview.version })}
                </span>
                <span className="tabular-nums">{pinLine(preview.commitSha)}</span>
              </div>
            </div>
            <div className="flex flex-col gap-0.5 text-footnote text-text-muted">
              <span>{addsLine(preview.capabilities)}</span>
              {preview.runsCode ? <span>{t.runsCode}</span> : null}
            </div>
            {preview.unsupportedFeature ? (
              <p role="alert" className="text-footnote text-danger">
                {format(t.unsupportedFeature, { feature: preview.unsupportedFeature })}
              </p>
            ) : null}
            <div className="flex flex-col gap-1">
              <span className="text-footnote font-bold text-text-tertiary">{t.hosts}</span>
              <ul aria-label={t.hosts} className="flex flex-col gap-0.5">
                {preview.hosts.length === 0 ? (
                  <li className="text-callout text-text-muted">{t.infoNoHosts}</li>
                ) : (
                  preview.hosts.map((host) => (
                    <li key={host} className="truncate text-callout text-text">
                      {host}
                    </li>
                  ))
                )}
              </ul>
            </div>
            <p className="text-footnote text-text-muted">{t.trust}</p>
            <ErrorLine error={error} />
          </div>
        </DialogContent>
      ) : null}
    </Dialog>
  );
}

/** Extensions tab: add registries or single extensions, see what's installed, browse registries. */
export function Extensions() {
  const qc = useQueryClient();
  const catalog = useCatalog();
  const actions = useExtensionActions();
  const [link, setLink] = useState("");
  const [adding, setAdding] = useState(false);
  const [addError, setAddError] = useState<string | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);

  const add = (raw: string) => {
    const url = raw.trim();
    if (!url || adding) return;
    setAddError(null);
    setAdding(true);
    const done = isRegistryLink(url)
      ? commands.extensionRegistryAdd(url).then((r) => {
          qc.setQueryData(queryKeys.registry(r.url), r);
          void qc.invalidateQueries({ queryKey: queryKeys.registries });
          setLink("");
        })
      : commands.extensionPreview(url).then(setPreview);
    done.catch((err: unknown) => setAddError(errorMessage(err))).finally(() => setAdding(false));
  };

  const submit = (e: FormEvent) => {
    e.preventDefault();
    add(link);
  };

  /** Pasting a link adds it straight away. */
  const paste = (e: ClipboardEvent<HTMLInputElement>) => {
    const text = e.clipboardData.getData("text").trim();
    if (!text) return;
    e.preventDefault();
    setLink(text);
    add(text);
  };

  const removeRegistry = (url: string) => {
    commands
      .extensionRegistryRemove(url)
      .then((next) => {
        qc.setQueryData(queryKeys.registries, next);
        qc.removeQueries({ queryKey: queryKeys.registry(url) });
      })
      .catch((err: unknown) => setAddError(errorMessage(err)));
  };

  const installPreview = () => {
    if (!preview) return;
    void actions.install("preview", preview).then((ok) => {
      if (ok) {
        setPreview(null);
        setLink("");
      }
    });
  };

  const { extensions, registries, installed, sections } = catalog;
  const loadingInstalled = extensions.isPending;
  const nothing = !loadingInstalled && !registries.isPending && installed.length === 0 && sections.length === 0;

  return (
    <Page title={t.title}>
      <div className="mx-auto flex w-full max-w-[880px] flex-col gap-8">
        <form className="flex flex-col gap-2" onSubmit={submit}>
          <p className="px-4 text-footnote text-text-muted">{t.hint}</p>
          <div className="flex items-center gap-2">
            <Input
              value={link}
              onChange={(e) => setLink(e.target.value)}
              onPaste={paste}
              leading={<Icon as={Link2} />}
              placeholder={t.linkPlaceholder}
              aria-label={t.linkLabel}
              spellCheck={false}
              autoComplete="off"
              shape="capsule"
              className="flex-1"
            />
            <Button variant="secondary" type="submit" disabled={adding || link.trim() === ""}>
              {adding ? t.adding : t.add}
            </Button>
          </div>
          <div className="px-4">
            <ErrorLine error={addError} />
          </div>
        </form>

        {nothing ? (
          <EmptyState icon={Blocks} title={t.emptyTitle} body={t.emptyBody} />
        ) : (
          <>
            <Section title={t.installedTitle}>
              {extensions.isError ? (
                <div className="flex items-center gap-3 px-4 py-3">
                  <p role="alert" className="min-w-0 flex-1 text-footnote text-danger">
                    {t.loadError}
                  </p>
                  <Button size="sm" variant="secondary" onClick={() => void extensions.refetch()}>
                    {strings.common.retry}
                  </Button>
                </div>
              ) : loadingInstalled ? (
                <RowsSkeleton />
              ) : installed.length === 0 ? (
                <p className="px-4 py-3 text-callout text-text-tertiary">{t.noInstalled}</p>
              ) : (
                <ul aria-label={t.installedTitle} className="flex flex-col divide-y divide-border py-1">
                  {installed.map((item) => (
                    <ExtensionRow
                      key={item.key}
                      item={item}
                      busy={actions.busy}
                      error={actions.errors[item.key]}
                      onInstall={(p) => void actions.install(item.key, p)}
                    />
                  ))}
                </ul>
              )}
            </Section>
            {sections.map((section) => (
              <RegistryBlock
                key={section.url}
                section={section}
                busy={actions.busy}
                errors={actions.errors}
                onInstall={(key, p) => void actions.install(key, p)}
                onRemove={() => removeRegistry(section.url)}
              />
            ))}
          </>
        )}
      </div>
      <PreviewDialog
        preview={preview}
        installing={actions.busy === "preview"}
        error={actions.errors["preview"]}
        onInstall={installPreview}
        onClose={() => {
          if (actions.busy !== "preview") setPreview(null);
        }}
      />
    </Page>
  );
}
