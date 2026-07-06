import { appToast } from "./AppToast.js";
import type { ToastVariant } from "./toastStore.js";

interface ToastOptions {
  description?: string;
  action?: { label: string; onClick: () => void };
}

function show(variant: ToastVariant) {
  return (title: string, opts?: ToastOptions) =>
    appToast({ title, description: opts?.description, action: opts?.action, variant });
}

/** sonner-compatible facade over the app toast system. */
export const toast = Object.assign(show("default"), {
  success: show("success"),
  error: show("error"),
  info: show("info"),
  message: show("default"),
  dismiss: (id?: string | number) => appToast.dismiss(id)
});
