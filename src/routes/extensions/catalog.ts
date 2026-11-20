import { useQueries, useQueryClient } from "@tanstack/react-query";
import { useState } from "react";
import type {
  AppError,
  Capability,
  ExtensionInfo_Serialize as ExtensionInfo,
  ExtensionPreview_Serialize as Preview,
  RegistryPreview_Serialize as Registry,
} from "@/lib/bindings";
import { commands } from "@/lib/bindings";
import { errorMessage } from "@/lib/errors";
import { queryKeys, useExtensions, useRegistries } from "@/lib/queries";
import { isTauri } from "@/lib/window";

/** One extension as the Extensions page shows it: installed, listed in a registry, or both. */
export type CatalogItem = {
  /** Route key: the extension id, or the registry entry when it couldn't be read. */
  key: string;
  id: string | null;
  name: string;
  version: string | null;
  author: string | null;
  description: string | null;
  icon: string | null;
  tags: string[];
  category: string | null;
  types: string[];
  capabilities: Capability[];
  /** `null` when unknown (an entry that couldn't be read). */
  runsCode: boolean | null;
  hosts: string[];
  homepage: string | null;
  /** The registry that lists it, if any. */
  registry: string | null;
  installed: ExtensionInfo | null;
  /** Installable version from a registry. */
  preview: Preview | null;
  /** Why a registry entry can't be installed. */
  error: AppError | null;
  /** A required runtime feature Unison refuses: it can't be installed or turned on. */
  unsupportedFeature: string | null;
};

export type InstallState = "new" | "installed" | "update" | "unavailable";

export function installState(item: CatalogItem): InstallState {
  // A version that needs a refused runtime feature is never offered.
  if (item.preview?.unsupportedFeature) return item.installed ? "installed" : "unavailable";
  if (item.installed) {
    return item.preview && item.preview.version !== item.installed.version ? "update" : "installed";
  }
  return item.preview ? "new" : "unavailable";
}

/** A short name for a registry link: `owner/repo` on GitHub, otherwise the host. */
export function registryName(url: string): string {
  try {
    const u = new URL(url);
    const parts = u.pathname.split("/").filter(Boolean);
    if ((u.hostname === "github.com" || u.hostname === "raw.githubusercontent.com") && parts.length >= 2) {
      return `${parts[0]}/${parts[1]}`;
    }
    return u.hostname;
  } catch {
    return url;
  }
}

/** A registry.json link lists several extensions; anything else is one extension. */
export const isRegistryLink = (url: string) => /\/registry\.json\/?(?:[?#].*)?$/i.test(url);

function fromInstalled(e: ExtensionInfo): CatalogItem {
  return {
    key: e.id,
    id: e.id,
    name: e.name,
    version: e.version,
    author: e.author,
    description: e.description,
    icon: e.icon,
    tags: [],
    category: null,
    types: e.types,
    capabilities: e.capabilities,
    runsCode: e.runsCode,
    hosts: e.hosts,
    homepage: e.homepage,
    registry: null,
    installed: e,
    preview: null,
    error: null,
    unsupportedFeature: e.unsupportedFeature,
  };
}

function fromRegistry(registry: Registry, installed: readonly ExtensionInfo[]): CatalogItem[] {
  return registry.items.map((item) => {
    const p = item.preview;
    const l = item.listed;
    const id = p?.id ?? l.id;
    return {
      key: p?.id ?? item.entry,
      id,
      name: p?.name ?? l.name ?? (item.entry || l.id || ""),
      version: p?.version ?? l.version,
      author: p?.author ?? null,
      description: p?.description ?? l.description,
      icon: p?.icon ?? null,
      tags: l.tags,
      category: l.category,
      types: p?.types ?? [],
      capabilities: p?.capabilities ?? [],
      runsCode: p ? p.runsCode : null,
      hosts: p?.hosts ?? [],
      homepage: p?.homepage ?? null,
      registry: registry.url,
      installed: (p && installed.find((e) => e.id === p.id)) ?? null,
      preview: p,
      error: item.error,
      unsupportedFeature: p?.unsupportedFeature ?? null,
    };
  });
}

export type RegistrySection = {
  url: string;
  loading: boolean;
  error: unknown;
  refetch: () => void;
  items: CatalogItem[];
};

/** Installed extensions plus everything the saved registries list. */
export function useCatalog() {
  const extensions = useExtensions();
  const registries = useRegistries();
  const urls = registries.data ?? [];
  const reads = useQueries({
    queries: urls.map((url) => ({
      queryKey: queryKeys.registry(url),
      queryFn: () => commands.extensionRegistry(url),
      enabled: isTauri(),
      staleTime: 10 * 60_000,
    })),
  });
  const installed = extensions.data ?? [];

  const sections: RegistrySection[] = urls.map((url, i) => {
    const q = reads[i];
    return {
      url,
      loading: q?.isPending ?? true,
      error: q?.error ?? null,
      refetch: () => void q?.refetch(),
      items: q?.data ? fromRegistry(q.data, installed) : [],
    };
  });
  const listed = sections.flatMap((s) => s.items);

  // Installed rows pick up a registry's tags, category and newer version.
  const installedItems = installed.map((e) => {
    const match = listed.find((l) => l.id === e.id);
    return match
      ? {
          ...fromInstalled(e),
          tags: match.tags,
          category: match.category,
          registry: match.registry,
          preview: match.preview,
          unsupportedFeature: e.unsupportedFeature ?? match.unsupportedFeature,
        }
      : fromInstalled(e);
  });

  const find = (key: string): CatalogItem | null =>
    installedItems.find((i) => i.key === key) ?? listed.find((i) => i.key === key) ?? null;

  return {
    extensions,
    registries,
    installed: installedItems,
    sections,
    find,
    /** Still reading something that could contain `key`. */
    loading: extensions.isPending || registries.isPending || sections.some((s) => s.loading),
  };
}

/** Installs, updates, toggles and removes extensions; keeps the list and Search in sync. */
export function useExtensionActions() {
  const qc = useQueryClient();
  const [busy, setBusy] = useState<string | null>(null);
  const [errors, setErrors] = useState<Record<string, string>>({});

  const applied = (next: ExtensionInfo[]) => {
    qc.setQueryData(queryKeys.extensions, next);
    // Results of new, turned-off or removed extensions change Search.
    void qc.invalidateQueries({ queryKey: ["extensions", "search"] });
  };

  const run = (key: string, action: () => Promise<ExtensionInfo[]>) => {
    if (busy) return Promise.resolve(false);
    setBusy(key);
    setErrors((prev) => Object.fromEntries(Object.entries(prev).filter(([k]) => k !== key)));
    return action()
      .then((next) => {
        applied(next);
        return true;
      })
      .catch((err: unknown) => {
        setErrors((prev) => ({ ...prev, [key]: errorMessage(err) }));
        return false;
      })
      .finally(() => setBusy(null));
  };

  return {
    busy,
    errors,
    install: (key: string, p: Preview) => run(key, () => commands.extensionInstall(p.repoUrl, p.commitSha)),
    setEnabled: (key: string, id: string, on: boolean) => run(key, () => commands.extensionSetEnabled(id, on)),
    remove: (key: string, id: string) => run(key, () => commands.extensionRemove(id)),
  };
}
