import {
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
import type { ReactNode } from "react";
import { NavLink } from "react-router";
import { Icon, Tooltip } from "@/components/ui";
import { strings } from "@/i18n";
import { cn } from "@/lib/cn";

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

function SidebarLink({ item, collapsed }: { item: NavItem; collapsed: boolean }) {
  const link = (
    <NavLink
      to={item.to}
      {...(item.end ? { end: true } : {})}
      {...(collapsed ? { "aria-label": item.label } : {})}
      className={({ isActive }) =>
        cn(
          "u-tone relative flex h-8 items-center gap-2.5 rounded-control text-body",
          collapsed ? "w-10 justify-center" : "px-2.5",
          isActive
            ? "bg-fill-tertiary font-medium text-text"
            : "text-text-muted hover:bg-fill-quaternary hover:text-text",
        )
      }
    >
      {({ isActive }) => (
        <>
          {isActive ? (
            <span
              aria-hidden="true"
              className="absolute top-2 bottom-2 left-0 w-0.5 rounded-full bg-accent"
            />
          ) : null}
          <Icon as={item.icon} size={16} />
          {collapsed ? null : <span className="truncate">{item.label}</span>}
        </>
      )}
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

function Group({
  label,
  collapsed,
  children,
}: {
  label: string;
  collapsed: boolean;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-0.5">
      {collapsed ? (
        <div aria-hidden="true" className="mx-auto my-2 h-px w-6 bg-border" />
      ) : (
        <h2 className="px-2.5 pt-3 pb-1 text-footnote font-semibold text-text-tertiary">{label}</h2>
      )}
      {children}
    </div>
  );
}

/** Floating glass sidebar (240 px, 64 px collapsed). */
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
      {/* Top strip doubles as a drag region, like a title bar. */}
      <div data-tauri-drag-region className="flex h-10 shrink-0 items-center px-4">
        {collapsed ? null : (
          <span data-tauri-drag-region className="text-title3 text-text">
            {strings.app.name}
          </span>
        )}
      </div>

      <div className={cn("flex flex-col gap-1", collapsed ? "items-center" : "px-2")}>
        <Group label={strings.nav.listen} collapsed={collapsed}>
          {LISTEN.map((item) => (
            <SidebarLink key={item.to} item={item} collapsed={collapsed} />
          ))}
        </Group>
        <Group label={strings.nav.library} collapsed={collapsed}>
          {LIBRARY.map((item) => (
            <SidebarLink key={item.to} item={item} collapsed={collapsed} />
          ))}
        </Group>
      </div>

      {collapsed ? (
        <div className="flex-1" />
      ) : (
        <div className="mt-1 flex min-h-0 flex-1 flex-col px-2">
          <h2 className="px-2.5 pt-3 pb-1 text-footnote font-semibold text-text-tertiary">
            {strings.nav.yourPlaylists}
          </h2>
          <div className="min-h-0 flex-1 overflow-y-auto">
            <p className="px-2.5 py-1 text-callout text-text-tertiary">{strings.nav.noPlaylists}</p>
          </div>
        </div>
      )}

      <div
        className={cn(
          "flex flex-col gap-0.5 border-t border-border py-2",
          collapsed ? "w-full items-center" : "px-2",
        )}
      >
        {devItems.map((item) => (
          <SidebarLink key={item.to} item={item} collapsed={collapsed} />
        ))}
        <SidebarLink
          item={{ to: "/settings", label: strings.nav.settings, icon: Settings }}
          collapsed={collapsed}
        />
        {collapsed ? null : (
          <p className="flex items-center gap-2 px-2.5 pt-1 text-footnote text-text-tertiary">
            <span aria-hidden="true" className="size-1.5 rounded-full bg-text-tertiary" />
            {strings.account.notConnected}
          </p>
        )}
      </div>
    </nav>
  );
}
