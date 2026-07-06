import { useCallback, useSyncExternalStore } from "react";
import {
  getResolvedTheme,
  getThemeSetting,
  setThemeSetting,
  subscribeToTheme,
  type ResolvedTheme,
  type ThemeSetting
} from "../lib/theme.js";

export type UseThemeResult = {
  setting: ThemeSetting;
  resolved: ResolvedTheme;
  setSetting: (next: ThemeSetting) => void;
};

/** Shared theme state; safe to mount from multiple components. */
export function useTheme(): UseThemeResult {
  const setting = useSyncExternalStore(subscribeToTheme, getThemeSetting);
  const resolved = useSyncExternalStore(subscribeToTheme, getResolvedTheme);
  const setSetting = useCallback((next: ThemeSetting) => setThemeSetting(next), []);
  return { setting, resolved, setSetting };
}
