import { create } from "zustand";

export type ToastVariant = "default" | "success" | "error" | "info" | "loading";

/** Inline text link on the right of a plain description row (no background). */
export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface ToastItem {
  id: string;
  title: string;
  description?: string;
  variant: ToastVariant;
  /** Render the description as a click-to-copy code block. */
  copyable: boolean;
  action?: ToastAction;
  /** Auto-dismiss delay in ms; Infinity keeps it until dismissed. */
  duration: number;
}

interface ToastState {
  toasts: ToastItem[];
  /**
   * Add a toast and return the id it is shown under.
   * - Same id as a visible toast: replaced IN PLACE (loading -> result), keeping
   *   its slot and its React key, so the card updates without replaying its
   *   enter animation.
   * - `dedupe` and an identical visible toast (variant, title, description):
   *   that toast is refreshed in place instead of stacking a copy, so repeating
   *   an action doesn't slide in a pile of identical cards.
   * - Otherwise appended as the newest toast.
   */
  add: (item: ToastItem, options?: { dedupe?: boolean }) => string;
  /** Remove one toast by id, or all when id is omitted. */
  remove: (id?: string) => void;
}

/** Cap concurrent toasts so a burst can't fill the screen. */
export const MAX_TOASTS = 4;

function isSameContent(a: ToastItem, b: ToastItem): boolean {
  return a.variant === b.variant && a.title === b.title && a.description === b.description;
}

export const useToastStore = create<ToastState>((set, get) => ({
  toasts: [],
  add: (item, options) => {
    const { toasts } = get();
    const target =
      toasts.find((t) => t.id === item.id) ??
      (options?.dedupe ? toasts.find((t) => isSameContent(t, item)) : undefined);
    if (target) {
      // A fresh object even when nothing changed, so the card sees a new toast
      // and restarts its auto-dismiss timer.
      const next = { ...item, id: target.id };
      set({ toasts: toasts.map((t) => (t.id === target.id ? next : t)) });
      return target.id;
    }
    // Newest last so it stacks closest to the bottom-right corner.
    set({ toasts: [...toasts, item].slice(-MAX_TOASTS) });
    return item.id;
  },
  remove: (id) =>
    set((s) => ({
      toasts: id == null ? [] : s.toasts.filter((t) => t.id !== id)
    }))
}));

let idCounter = 0;
export function nextToastId(): string {
  idCounter += 1;
  return `toast-${idCounter}`;
}
