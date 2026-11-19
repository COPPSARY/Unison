import { useEffect } from "react";
import { useNavigate } from "react-router";
import { usePlayerStore } from "@/stores/player";

/** True when the event comes from a text field, where shortcuts must not fire. */
function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable) return true;
  const tag = target.tagName;
  if (tag === "TEXTAREA" || tag === "SELECT") return true;
  if (tag !== "INPUT") return false;
  const type = (target as HTMLInputElement).type;
  return !["checkbox", "radio", "range", "button", "submit"].includes(type);
}

/**
 * Global keyboard shortcuts (spec: keyboard-first).
 * Ctrl+K search · Ctrl+L liked songs · Ctrl+, settings · Alt+←/→ history · Space play/pause.
 */
export function useShortcuts() {
  const navigate = useNavigate();
  const togglePlay = usePlayerStore((s) => s.togglePlay);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.defaultPrevented) return;
      const ctrl = e.ctrlKey || e.metaKey;

      if (ctrl && !e.altKey && !e.shiftKey) {
        const key = e.key.toLowerCase();
        const target = key === "k" ? "/search" : key === "l" ? "/library/liked" : key === "," ? "/settings" : null;
        if (target) {
          e.preventDefault();
          void navigate(target);
        }
        return;
      }

      if (e.altKey && !ctrl && (e.key === "ArrowLeft" || e.key === "ArrowRight")) {
        e.preventDefault();
        void navigate(e.key === "ArrowLeft" ? -1 : 1);
        return;
      }

      if (e.key === " " && !ctrl && !e.altKey && !isTyping(e.target)) {
        // Space on a focused button/link activates it instead.
        if (e.target instanceof HTMLElement && e.target.closest("button, a, [role='slider']")) return;
        e.preventDefault();
        togglePlay();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate, togglePlay]);
}
