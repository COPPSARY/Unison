import { getCurrentWindow } from "@tauri-apps/api/window";

/** True inside the Tauri webview (false in a plain browser / tests). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function run(action: (w: ReturnType<typeof getCurrentWindow>) => Promise<unknown>) {
  if (!isTauri()) return;
  await action(getCurrentWindow());
}

/** Window chrome actions for the custom title bar. No-ops outside Tauri. */
export const windowControls = {
  minimize: () => run((w) => w.minimize()),
  toggleMaximize: () => run((w) => w.toggleMaximize()),
  close: () => run((w) => w.close()),
  /** Subscribes to maximise changes; returns an unsubscribe function. */
  onMaximizedChange(cb: (maximized: boolean) => void): () => void {
    if (!isTauri()) return () => undefined;
    const w = getCurrentWindow();
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const sync = () => void w.isMaximized().then((m) => !disposed && cb(m));
    sync();
    void w.onResized(sync).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  },
};
