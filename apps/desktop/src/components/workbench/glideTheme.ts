import { useMemo } from "react";
import type { Theme } from "@glideapps/glide-data-grid";
import { useTheme } from "../../hooks/useTheme.js";

// Maps the app's CSS design tokens onto a glide-data-grid theme so the canvas
// grid matches the surrounding UI in both light and dark. The tokens live as
// "R G B" channel triples on :root / .dark (see styles.css); we read the
// resolved values and rebuild the theme whenever the resolved theme flips.

type Channels = [number, number, number];

function readChannels(styles: CSSStyleDeclaration, variable: string): Channels {
  const raw = styles.getPropertyValue(variable).trim();
  const parts = raw.split(/\s+/).map((value) => Number(value));
  return [parts[0] ?? 0, parts[1] ?? 0, parts[2] ?? 0];
}

function rgb([r, g, b]: Channels): string {
  return `rgb(${r}, ${g}, ${b})`;
}

function rgba([r, g, b]: Channels, alpha: number): string {
  return `rgba(${r}, ${g}, ${b}, ${alpha})`;
}

/**
 * Builds a glide-data-grid theme from the live CSS tokens. Recomputed on theme
 * change; safe to call during SSR-less browser render (guards for no window).
 */
export function useGlideTheme(): Partial<Theme> {
  const { resolved } = useTheme();
  return useMemo<Partial<Theme>>(() => {
    // `resolved` is the recompute trigger: the .dark class is already applied by
    // the time it changes, so getComputedStyle reads the correct token values.
    void resolved;
    if (typeof window === "undefined") {
      return {};
    }
    const styles = getComputedStyle(document.documentElement);
    const ink = readChannels(styles, "--ink");
    const inkMuted = readChannels(styles, "--ink-muted");
    const inkFaint = readChannels(styles, "--ink-faint");
    const surface = readChannels(styles, "--surface");
    const raised = readChannels(styles, "--raised");
    const edge = readChannels(styles, "--edge");
    const edgeStrong = readChannels(styles, "--edge-strong");
    const accent = readChannels(styles, "--accent");
    const accentInk = readChannels(styles, "--accent-ink");
    const warning = readChannels(styles, "--warning");
    const fontFamily = styles.getPropertyValue("--font-ui").trim() || "Inter, sans-serif";

    return {
      accentColor: rgb(accent),
      accentFg: rgb(accentInk),
      accentLight: rgba(accent, 0.14),
      textDark: rgb(ink),
      textMedium: rgb(inkMuted),
      textLight: rgb(inkFaint),
      textBubble: rgb(ink),
      bgIconHeader: rgb(inkMuted),
      fgIconHeader: rgb(raised),
      textHeader: rgb(inkMuted),
      textGroupHeader: rgb(inkMuted),
      textHeaderSelected: rgb(ink),
      bgCell: rgb(raised),
      bgCellMedium: rgb(surface),
      bgHeader: rgb(surface),
      bgHeaderHasFocus: rgb(edge),
      bgHeaderHovered: rgb(surface),
      bgBubble: rgb(surface),
      bgBubbleSelected: rgb(raised),
      bgSearchResult: rgba(warning, 0.18),
      borderColor: rgb(edge),
      horizontalBorderColor: rgb(edge),
      drilldownBorder: rgb(edgeStrong),
      linkColor: rgb(accent),
      cellHorizontalPadding: 10,
      cellVerticalPadding: 6,
      headerFontStyle: "600 12px",
      baseFontStyle: "13px",
      markerFontStyle: "11px",
      fontFamily,
      editorFontSize: "13px"
    };
  }, [resolved]);
}
