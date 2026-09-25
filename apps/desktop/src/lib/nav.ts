export type ScreenId =
  | "dashboard"
  | "sources"
  | "tables"
  | "settings";

import type { TranslationKey } from "../i18n/translations.js";

export type NavItem = {
  id: ScreenId;
  /** Translation key for the display label; resolved at render via t(). */
  labelKey: TranslationKey;
  screen: ScreenId;
};

// Pure data; the sidebar maps ids to icons so nav stays framework-free.
export const NAV: readonly NavItem[] = [
  { id: "dashboard", labelKey: "nav.dashboard", screen: "dashboard" },
  { id: "sources", labelKey: "nav.sources", screen: "sources" },
  { id: "tables", labelKey: "nav.tables", screen: "tables" },
  { id: "settings", labelKey: "nav.settings", screen: "settings" }
] as const;
