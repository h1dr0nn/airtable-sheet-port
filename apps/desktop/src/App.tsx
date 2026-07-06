import { useState, type ComponentType } from "react";
import { ToastViewport } from "@sheet-port/ui";
import { CommandPalette } from "./components/CommandPalette.js";
import { Sidebar } from "./components/Sidebar.js";
import { Titlebar } from "./components/Titlebar.js";
import { useTheme } from "./hooks/useTheme.js";
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
  const [screen, setScreen] = useState<ScreenId>("dashboard");
  const [isPaletteOpen, setIsPaletteOpen] = useState(false);
  const Screen = SCREENS[screen];

  return (
    <div className="flex h-screen flex-col bg-bg font-sans text-ink">
      <Titlebar onNavigate={setScreen} onOpenPalette={() => setIsPaletteOpen(true)} />
      <div className="flex min-h-0 flex-1">
        <Sidebar active={screen} onNavigate={setScreen} />
        <main className="min-w-0 flex-1 overflow-y-auto">
          <div className="mx-auto max-w-6xl px-8 py-8">
            <Screen onNavigate={setScreen} />
          </div>
        </main>
      </div>
      <CommandPalette open={isPaletteOpen} onOpenChange={setIsPaletteOpen} onNavigate={setScreen} />
      <ToastViewport />
    </div>
  );
}
