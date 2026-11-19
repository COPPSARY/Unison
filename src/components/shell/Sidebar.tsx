import {
  ChevronDown,
  Disc3,
  Heart,
  House,
  ListMusic,
  MicVocal,
  Palette,
  Search,
  Settings,
  type LucideIcon,
} from "lucide-react";
import { useId, type ReactNode } from "react";
import { NavLink } from "react-router";
import { Icon, Tooltip } from "@/components/ui";
import { format, strings } from "@/i18n";
import { cn } from "@/lib/cn";
import { useAccounts } from "@/lib/queries";
import { useSyncStore } from "@/stores/sync";
import { useUiStore, type SidebarSection } from "@/stores/ui";

type NavItem = { to: string; label: string; icon: LucideIcon; end?: boolean };

const LISTEN: NavItem[] = [
  { to: "/", label: strings.nav.home, icon: House, end: true },
  { to: "/search", label: strings.nav.search, icon: Search },
];

const LIBRARY: NavItem[] = [
  { to: "/library/liked", label: strings.nav.liked, icon: Heart },
  { to: "/library/albums", label: strings.nav.albums, icon: Disc3 },
  { to: "/library/playlists", label: strings.nav.playlists, icon: ListMusic },
  { to: "/library/artists", label: strings.nav.artists, icon: MicVocal },
];

/** Finder-style row: accent-tinted icon, primary label, grey rounded fill when selected. */
function SidebarLink({ item, collapsed }: { item: NavItem; collapsed: boolean }) {
  const link = (
    <NavLink
      to={item.to}
      {...(item.end ? { end: true } : {})}
      {...(collapsed ? { "aria-label": item.label } : {})}
      className={({ isActive }) =>
        cn(
          "u-tone flex h-7 items-center gap-2 rounded-[8px] text-body text-text",
          collapsed ? "w-10 justify-center" : "px-2",
          isActive ? "bg-fill-secondary" : "hover:bg-fill-quaternary",
        )
      }
    >
      <Icon as={item.icon} size={16} className="shrink-0 text-accent" />
      {collapsed ? null : <span className="truncate">{item.label}</span>}
    </NavLink>
  );
  return collapsed ? (
    <Tooltip content={item.label} side="right">
      {link}
    </Tooltip>
  ) : (
    link
  );
}

/** Section with a bold muted header; hovering the header reveals a collapse chevron. */
function Section({
  id,
  label,
  collapsed,
  children,
  grow = false,
}: {
  id: SidebarSection;
  label: string;
  collapsed: boolean;
  children: ReactNode;
  grow?: boolean;
}) {
  const open = useUiStore((st) => !st.sidebarCollapsed.includes(id));
  const toggle = useUiStore((st) => st.toggleSection);
  const bodyId = useId();

  if (collapsed) {
    return (
      <div className="flex flex-col items-center gap-0.5">
        <div aria-hidden="true" className="my-2 h-px w-6 bg-border" />
        {children}
      </div>
    );
  }

  return (
    <div className={cn("flex flex-col", grow && open && "min-h-0 flex-1")}>
      <h2 className="mt-3">
        <button
          type="button"
          aria-expanded={open}
          aria-controls={bodyId}
          onClick={() => toggle(id)}
          className="group u-tone flex h-6 w-full items-center justify-between rounded-[6px] px-2 text-left text-footnote font-bold text-text-tertiary"
        >
          {label}
          <Icon
            as={ChevronDown}
            size={12}
            className={cn(
              "opacity-0 transition-[opacity,transform] duration-(--spring-smooth-duration) ease-(--spring-smooth)",
              "group-hover:opacity-100 group-focus-visible:opacity-100",
              !open && "-rotate-90",
            )}
          />
        </button>
      </h2>
      <div
        id={bodyId}
        hidden={!open}
        className={cn("flex flex-col gap-px", grow && "min-h-0 flex-1 overflow-y-auto")}
      >
        {children}
      </div>
    </div>
  );
}

/** Floating glass sidebar, Finder-style (240 px, 64 px collapsed). */
function syncLine(st: { stage: keyof typeof strings.sync.stage; done: number; total: number | null } | undefined) {
  if (!st) return "";
  const stage = strings.sync.stage[st.stage];
  if (st.total === null || st.done === 0) return stage;
  return format(strings.sync.progress, { stage, done: count.format(st.done), total: count.format(st.total) });
}

/** Footer line: connected accounts, or a muted "No account connected". */
const count = new Intl.NumberFormat();

function AccountStatusLine() {
  const accounts = useAccounts();
  const running = useSyncStore((st) => st.running);
  const connected = (accounts.data ?? []).filter((a) => a.status !== "notConnected");
  if (connected.length === 0) {
    return (
      <p className="flex items-center gap-2 px-2 pt-1.5 text-footnote text-text-tertiary">
        <span aria-hidden="true" className="size-1.5 rounded-full bg-text-tertiary" />
        {strings.account.notConnected}
      </p>
    );
  }
  return (
    <>
      {connected.map((a) => (
        <p key={a.provider} className="flex items-center gap-2 px-2 pt-1.5 text-footnote text-text-muted">
          <span
            aria-hidden="true"
            className={cn(
              "size-1.5 shrink-0 rounded-full",
              a.status === "needsReconnect" ? "bg-warning" : running[a.provider] ? "bg-accent" : "bg-success",
            )}
          />
          <span className="truncate tabular-nums">
            {running[a.provider]
              ? syncLine(running[a.provider])
              : format(strings.account.connectedTo, {
                  provider: strings.source[a.provider],
                  name: a.displayName ?? "",
                })}
          </span>
        </p>
      ))}
    </>
  );
}

export function Sidebar({ collapsed }: { collapsed: boolean }) {
  const devItems: NavItem[] = import.meta.env.DEV
    ? [{ to: "/dev/ui", label: strings.nav.uiKit, icon: Palette }]
    : [];

  return (
    <nav
      aria-label={strings.nav.sidebar}
      className={cn(
        "glass absolute top-2 bottom-[88px] left-2 z-30 flex flex-col rounded-panel",
        collapsed ? "w-16 items-center" : "w-60",
      )}
    >
      {/* Empty top strip: drag region aligned with the title bar (where macOS has its lights). */}
      <div data-tauri-drag-region className="h-8 w-full shrink-0" />

      <div className={cn("flex min-h-0 flex-1 flex-col", collapsed ? "items-center" : "px-2")}>
        <Section id="listen" label={strings.nav.listen} collapsed={collapsed}>
          {LISTEN.map((item) => (
            <SidebarLink key={item.to} item={item} collapsed={collapsed} />
          ))}
        </Section>
        <Section id="library" label={strings.nav.library} collapsed={collapsed}>
          {LIBRARY.map((item) => (
            <SidebarLink key={item.to} item={item} collapsed={collapsed} />
          ))}
        </Section>
        {collapsed ? null : (
          <Section id="playlists" label={strings.nav.yourPlaylists} collapsed={collapsed} grow>
            <p className="px-2 py-1 text-callout text-text-tertiary">{strings.nav.noPlaylists}</p>
          </Section>
        )}
      </div>

      <div
        className={cn(
          "flex w-full flex-col gap-px border-t border-border py-2",
          collapsed ? "items-center" : "px-2",
        )}
      >
        {devItems.map((item) => (
          <SidebarLink key={item.to} item={item} collapsed={collapsed} />
        ))}
        <SidebarLink
          item={{ to: "/settings", label: strings.nav.settings, icon: Settings }}
          collapsed={collapsed}
        />
        {collapsed ? null : <AccountStatusLine />}
      </div>
    </nav>
  );
}
