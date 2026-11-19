import { useEffect, useRef, useState } from "react";
import { Outlet, useLocation } from "react-router";
import { PlayerBar } from "@/components/player/PlayerBar";
import { cn } from "@/lib/cn";
import { useMediaQuery } from "@/lib/useMediaQuery";
import { Sidebar } from "./Sidebar";
import { TitleBar } from "./TitleBar";
import { useShortcuts } from "./useShortcuts";

/** Below this window width the sidebar collapses to icons (spec §3). */
export const COLLAPSE_QUERY = "(max-width: 1099px)";

/** App frame: floating glass sidebar + player bar over a scrolling content layer. */
export function Shell() {
  const collapsed = useMediaQuery(COLLAPSE_QUERY);
  const [scrolled, setScrolled] = useState(false);
  const scrollRef = useRef<HTMLElement>(null);
  const { pathname } = useLocation();
  useShortcuts();

  // New page: start at the top.
  useEffect(() => {
    if (scrollRef.current) scrollRef.current.scrollTop = 0;
    setScrolled(false);
  }, [pathname]);

  return (
    <div className="relative h-full overflow-hidden bg-background">
      <Sidebar collapsed={collapsed} />
      <div className={cn("absolute inset-y-0 right-0", collapsed ? "left-[72px]" : "left-[248px]")}>
        <TitleBar scrolled={scrolled} />
        <main
          ref={scrollRef}
          onScroll={(e) => setScrolled(e.currentTarget.scrollTop > 4)}
          className="h-full overflow-y-auto pt-10 pb-[96px]"
        >
          <Outlet />
        </main>
      </div>
      <PlayerBar />
    </div>
  );
}
