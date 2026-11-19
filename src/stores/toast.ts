import { create } from "zustand";

export type ToastTone = "default" | "danger";

export type ToastItem = {
  id: number;
  title: string;
  description?: string;
  tone: ToastTone;
  /** Optional single action, e.g. Undo. */
  action?: { label: string; onAction: () => void };
};

type ToastState = {
  toasts: ToastItem[];
  push: (toast: Omit<ToastItem, "id" | "tone"> & { tone?: ToastTone }) => number;
  dismiss: (id: number) => void;
};

let nextId = 1;

export const useToastStore = create<ToastState>((set) => ({
  toasts: [],
  push: ({ tone = "default", ...rest }) => {
    const id = nextId++;
    set((s) => ({ toasts: [...s.toasts, { id, tone, ...rest }].slice(-3) }));
    return id;
  },
  dismiss: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
}));

/** Convenience for non-React callers (event listeners, query callbacks). */
export const toast = (t: Parameters<ToastState["push"]>[0]) => useToastStore.getState().push(t);
