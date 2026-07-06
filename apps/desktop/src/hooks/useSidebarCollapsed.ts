import { useCallback, useState } from "react";

/** Persisted collapsed/expanded state for the sidebar icon rail. */
export const SIDEBAR_COLLAPSED_STORAGE_KEY = "sheet-port-sidebar-collapsed";

function readStoredCollapsed(): boolean {
  try {
    return window.localStorage.getItem(SIDEBAR_COLLAPSED_STORAGE_KEY) === "true";
  } catch {
    // Storage can be unavailable in hardened webviews; default to expanded.
    return false;
  }
}

export type UseSidebarCollapsedResult = {
  collapsed: boolean;
  toggle: () => void;
};

/** Sidebar collapse state, lifted to App and persisted across launches. */
export function useSidebarCollapsed(): UseSidebarCollapsedResult {
  const [collapsed, setCollapsed] = useState<boolean>(readStoredCollapsed);

  const toggle = useCallback(() => {
    setCollapsed((current) => {
      const next = !current;
      try {
        window.localStorage.setItem(SIDEBAR_COLLAPSED_STORAGE_KEY, String(next));
      } catch {
        // Persisting failed; the choice still applies for this session.
      }
      return next;
    });
  }, []);

  return { collapsed, toggle };
}
