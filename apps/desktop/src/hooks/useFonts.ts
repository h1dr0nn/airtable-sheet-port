import { useEffect } from "react";
import { FONT_FAMILY_ATTR, FONT_SCALE_ATTR } from "../lib/constants.js";
import type { FontFamily, FontScale } from "../lib/ipc.js";
import { useSettings } from "./useSettings.js";

/**
 * Applies the persisted font preferences to the document root so CSS can map
 * them to a size multiplier and font-family stack (see styles.css). Mount once
 * near the app root; it re-applies whenever the settings query updates. Until
 * settings load, the CSS defaults (normal + modern) remain in effect.
 */
export function useFonts(): void {
  const { data: settings } = useSettings();
  const scale: FontScale | undefined = settings?.fontScale;
  const family: FontFamily | undefined = settings?.fontFamily;

  useEffect(() => {
    if (scale === undefined) {
      return;
    }
    document.documentElement.setAttribute(FONT_SCALE_ATTR, scale);
  }, [scale]);

  useEffect(() => {
    if (family === undefined) {
      return;
    }
    document.documentElement.setAttribute(FONT_FAMILY_ATTR, family);
  }, [family]);
}
