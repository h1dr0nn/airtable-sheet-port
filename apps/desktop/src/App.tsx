import { useEffect, useState, type ComponentType } from "react";
import { AnimatedScreen, ToastViewport } from "@sheet-port/ui";
import { CommandPalette } from "./components/CommandPalette.js";
import { ErrorBoundary } from "./components/ErrorBoundary.js";
import { Sidebar } from "./components/Sidebar.js";
import { Titlebar } from "./components/Titlebar.js";
import { useFonts } from "./hooks/useFonts.js";
import { useSidebarCollapsed } from "./hooks/useSidebarCollapsed.js";
import { useTheme } from "./hooks/useTheme.js";
import { useUpdate } from "./hooks/useUpdate.js";
import type { ScreenId } from "./lib/nav.js";
import { Changes } from "./screens/Changes.js";
import { Dashboard } from "./screens/Dashboard.js";
import { DataSources } from "./screens/DataSources.js";
import { Settings } from "./screens/Settings.js";
import { Tables } from "./screens/Tables.js";

type ScreenProps = {
  onNavigate: (screen: ScreenId) => void;
};

const SCREENS: Record<ScreenId, ComponentType<ScreenProps>> = {
  dashboard: Dashboard,
  sources: DataSources,
  tables: Tables,
  changes: Changes,
  settings: Settings
};

export function App() {
  // Arms the theme store early so system-scheme changes apply app-wide,
  // even before the Settings screen is ever opened.
  useTheme();
  // Applies the persisted font-size/family preferences to the document root.
  useFonts();
  const [screen, setScreen] = useState<ScreenId>("dashboard");
  const [isPaletteOpen, setIsPaletteOpen] = useState(false);
  const { collapsed, toggle: toggleSidebar } = useSidebarCollapsed();
  const Screen = SCREENS[screen];

  // Silent launch check: no toast, no dialog. When a newer version is found the
  // Sidebar bottom cluster morphs into an update prompt (see update.available).
  const update = useUpdate();
  useEffect(() => {
    void update.check();
    // Run exactly once on mount; update.check is stable (useCallback).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <div className="flex h-screen flex-col bg-bg font-sans text-ink">
      <Titlebar
        onNavigate={setScreen}
        onOpenPalette={() => setIsPaletteOpen(true)}
        sidebarCollapsed={collapsed}
        onToggleSidebar={toggleSidebar}
      />
      <div className="flex min-h-0 flex-1">
        <Sidebar active={screen} onNavigate={setScreen} update={update} collapsed={collapsed} />
        <main className="min-w-0 flex-1 overflow-y-auto">
          <div className="app-scale mx-auto max-w-6xl px-8 py-8">
            <AnimatedScreen screenKey={screen}>
              <ErrorBoundary resetKey={screen}>
                <Screen onNavigate={setScreen} />
              </ErrorBoundary>
            </AnimatedScreen>
          </div>
        </main>
      </div>
      <CommandPalette open={isPaletteOpen} onOpenChange={setIsPaletteOpen} onNavigate={setScreen} />
      <ToastViewport />
    </div>
  );
}
