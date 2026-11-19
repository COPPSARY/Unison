import { ChevronLeft, ChevronRight, Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { useNavigate } from "react-router";
import { Icon, IconButton } from "@/components/ui";
import { strings } from "@/i18n";
import { cn } from "@/lib/cn";
import { windowControls } from "@/lib/window";

function WindowButton({
  label,
  onClick,
  danger = false,
  children,
}: {
  label: string;
  onClick: () => void;
  danger?: boolean;
  children: ReactNode;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      onClick={onClick}
      className={cn(
        "u-tone flex h-10 w-11 items-center justify-center text-text-muted",
        danger ? "hover:bg-danger hover:text-accent-foreground" : "hover:bg-fill-tertiary hover:text-text",
      )}
    >
      {children}
    </button>
  );
}

/** 40 px custom title bar: drag region, history buttons, Windows caption buttons. */
export function TitleBar({ scrolled }: { scrolled: boolean }) {
  const navigate = useNavigate();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => windowControls.onMaximizedChange(setMaximized), []);

  return (
    <header
      data-tauri-drag-region
      className={cn(
        "absolute inset-x-0 top-0 z-20 flex h-10 items-center pl-2",
        "transition-[background-color,box-shadow,backdrop-filter] duration-(--duration-tone)",
        scrolled && "glass glass-flat",
      )}
    >
      <div className="flex items-center gap-0.5">
        <IconButton
          label={strings.nav.back}
          size="sm"
          icon={<Icon as={ChevronLeft} />}
          onClick={() => void navigate(-1)}
        />
        <IconButton
          label={strings.nav.forward}
          size="sm"
          icon={<Icon as={ChevronRight} />}
          onClick={() => void navigate(1)}
        />
      </div>
      <div data-tauri-drag-region className="h-full flex-1" />
      <div className="flex self-start">
        <WindowButton label={strings.window.minimize} onClick={() => void windowControls.minimize()}>
          <Icon as={Minus} />
        </WindowButton>
        <WindowButton
          label={maximized ? strings.window.restore : strings.window.maximize}
          onClick={() => void windowControls.toggleMaximize()}
        >
          <Icon as={maximized ? Copy : Square} size={12} />
        </WindowButton>
        <WindowButton label={strings.window.close} danger onClick={() => void windowControls.close()}>
          <Icon as={X} />
        </WindowButton>
      </div>
    </header>
  );
}
