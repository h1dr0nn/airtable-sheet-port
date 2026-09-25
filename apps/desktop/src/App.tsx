import { useEffect, useState, type ComponentType } from "react";
import { AnimatedScreen, ToastViewport } from "@sheet-port/ui";
import { CommandPalette } from "./components/CommandPalette.js";
import { ErrorBoundary } from "./components/ErrorBoundary.js";
import { Sidebar } from "./components/Sidebar.js";
import { Titlebar } from "./components/Titlebar.js";
import { useFonts } from "./hooks/useFonts.js";
import { TranslationProvider } from "./i18n/useTranslation.js";
import { useSidebarCollapsed } from "./hooks/useSidebarCollapsed.js";
import { useTheme } from "./hooks/useTheme.js";
import { useUpdate } from "./hooks/useUpdate.js";
import { useUpdateRestartNotice } from "./hooks/useUpdateRestartNotice.js";
import type { ScreenId } from "./lib/nav.js";
import { Dashboard } from "./screens/Dashboard.js";
import { DataSources } from "./screens/DataSources.js";
import { Guide } from "./screens/Guide.js";
import { Settings } from "./screens/Settings.js";
import { Tables } from "./screens/Tables.js";

type ScreenProps = {
  onNavigate: (screen: ScreenId) => void;
};

const SCREENS: Record<ScreenId, ComponentType<ScreenProps>> = {
  dashboard: Dashboard,
  sources: DataSources,
  guide: Guide,
  tables: Tables,
  settings: Settings
};

/** Invisible: fires the post-update "restart your MCP clients" toast. Lives
 * inside TranslationProvider so the hook can translate the message. */
function UpdateRestartNotice() {
  useUpdateRestartNotice();
  return null;
}

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
  // The Workbench (Tables) fills the whole content area edge to edge; every
  // other screen keeps the centered, padded reading column.
  const isFullBleed = screen === "tables";

  // Silent launch check: no toast, no dialog. When a newer version is found the
  // Sidebar bottom cluster morphs into an update prompt (see update.available).
  const update = useUpdate();
  useEffect(() => {
    void update.check();
    // Run exactly once on mount; update.check is stable (useCallback).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return (
    <TranslationProvider>
      <div className="flex h-screen flex-col bg-bg font-sans text-ink">
        <Titlebar
          onNavigate={setScreen}
          onOpenPalette={() => setIsPaletteOpen(true)}
          sidebarCollapsed={collapsed}
          onToggleSidebar={toggleSidebar}
        />
        <div className="flex min-h-0 flex-1">
          <Sidebar active={screen} onNavigate={setScreen} update={update} collapsed={collapsed} />
          {isFullBleed ? (
            <main className="min-w-0 flex-1 overflow-hidden">
              <div className="app-scale h-full">
                <ErrorBoundary resetKey={screen}>
                  <Screen onNavigate={setScreen} />
                </ErrorBoundary>
              </div>
            </main>
          ) : (
            <main className="min-w-0 flex-1 overflow-y-auto">
              <div className="app-scale mx-auto max-w-6xl px-8 py-8">
                <AnimatedScreen screenKey={screen}>
                  <ErrorBoundary resetKey={screen}>
                    <Screen onNavigate={setScreen} />
                  </ErrorBoundary>
                </AnimatedScreen>
              </div>
            </main>
          )}
        </div>
        <CommandPalette open={isPaletteOpen} onOpenChange={setIsPaletteOpen} onNavigate={setScreen} />
        <UpdateRestartNotice />
        <ToastViewport />
      </div>
    </TranslationProvider>
  );
}
