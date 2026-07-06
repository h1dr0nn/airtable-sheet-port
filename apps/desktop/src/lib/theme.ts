export type ThemeSetting = "light" | "dark" | "system";
export type ResolvedTheme = "light" | "dark";

/** Kept in sync with the inline bootstrap script in index.html. */
export const THEME_STORAGE_KEY = "sheet-port-theme";

const DARK_SCHEME_QUERY = "(prefers-color-scheme: dark)";
const THEME_SETTINGS: readonly ThemeSetting[] = ["light", "dark", "system"];

function isThemeSetting(value: unknown): value is ThemeSetting {
  return typeof value === "string" && THEME_SETTINGS.includes(value as ThemeSetting);
}

function readStoredSetting(): ThemeSetting {
  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    return isThemeSetting(stored) ? stored : "system";
  } catch {
    // Storage can be unavailable in hardened webviews; default to system.
    return "system";
  }
}

function systemTheme(): ResolvedTheme {
  return window.matchMedia(DARK_SCHEME_QUERY).matches ? "dark" : "light";
}

export function resolveTheme(setting: ThemeSetting): ResolvedTheme {
  return setting === "system" ? systemTheme() : setting;
}

function applyResolvedTheme(theme: ResolvedTheme): void {
  const root = document.documentElement;
  root.classList.toggle("dark", theme === "dark");
  root.style.colorScheme = theme;
}

// Module-level store so every useTheme() consumer shares one source of truth.
let currentSetting: ThemeSetting = readStoredSetting();
const listeners = new Set<() => void>();
let isWatchingSystem = false;

function notifyListeners(): void {
  for (const listener of listeners) {
    listener();
  }
}

/** Live-updates the theme when the OS scheme changes and setting = system. */
function ensureSystemWatcher(): void {
  if (isWatchingSystem) {
    return;
  }
  isWatchingSystem = true;
  window.matchMedia(DARK_SCHEME_QUERY).addEventListener("change", () => {
    if (currentSetting !== "system") {
      return;
    }
    applyResolvedTheme(systemTheme());
    notifyListeners();
  });
}

export function getThemeSetting(): ThemeSetting {
  return currentSetting;
}

export function getResolvedTheme(): ResolvedTheme {
  return resolveTheme(currentSetting);
}

export function setThemeSetting(next: ThemeSetting): void {
  currentSetting = next;
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, next);
  } catch {
    // Persisting failed; the theme still applies for this session.
  }
  applyResolvedTheme(resolveTheme(next));
  notifyListeners();
}

export function subscribeToTheme(listener: () => void): () => void {
  ensureSystemWatcher();
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
