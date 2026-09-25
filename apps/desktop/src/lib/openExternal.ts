import { isTauri } from "./ipc.js";

/**
 * Opens an https URL in the system browser. Inside Tauri this goes through the
 * opener plugin (capabilities allow only the URLs the app links to); the
 * browser preview falls back to a new tab.
 */
export async function openExternal(url: string): Promise<void> {
  if (!isTauri) {
    window.open(url, "_blank", "noopener,noreferrer");
    return;
  }
  const { openUrl } = await import("@tauri-apps/plugin-opener");
  await openUrl(url);
}
