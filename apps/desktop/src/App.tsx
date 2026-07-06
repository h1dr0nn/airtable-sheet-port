import { useState, type ComponentType } from "react";
import { Sidebar } from "./components/Sidebar.js";
import { Titlebar } from "./components/Titlebar.js";
import type { ScreenId } from "./lib/nav.js";
import { AuditLog } from "./screens/AuditLog.js";
import { Changes } from "./screens/Changes.js";
import { Dashboard } from "./screens/Dashboard.js";
import { DataSources } from "./screens/DataSources.js";
import { Permissions } from "./screens/Permissions.js";
import { Tables } from "./screens/Tables.js";

type ScreenProps = {
  onNavigate: (screen: ScreenId) => void;
};

const SCREENS: Record<ScreenId, ComponentType<ScreenProps>> = {
  dashboard: Dashboard,
  sources: DataSources,
  tables: Tables,
  permissions: Permissions,
  changes: Changes,
  audit: AuditLog
};

export function App() {
  const [screen, setScreen] = useState<ScreenId>("dashboard");
  const Screen = SCREENS[screen];

  return (
    <div className="flex h-screen flex-col bg-bg text-ink">
      <Titlebar />
      <div className="flex min-h-0 flex-1">
        <Sidebar active={screen} onNavigate={setScreen} />
        <main className="min-w-0 flex-1 overflow-y-auto px-8 py-6">
          <Screen onNavigate={setScreen} />
        </main>
      </div>
    </div>
  );
}
