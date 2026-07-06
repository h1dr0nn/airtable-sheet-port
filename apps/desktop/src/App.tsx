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

const CORNER_POSITIONS = [
  "left-1.5 top-1",
  "right-1.5 top-1",
  "left-1.5 bottom-1",
  "right-1.5 bottom-1"
] as const;

/** Crosshair "+" registration marks at the corners of the content compartment. */
function CornerMarks() {
  return (
    <>
      {CORNER_POSITIONS.map((position) => (
        <span
          key={position}
          aria-hidden
          className={`pointer-events-none absolute z-10 select-none font-mono text-[10px] leading-none text-edge-strong ${position}`}
        >
          +
        </span>
      ))}
    </>
  );
}

export function App() {
  const [screen, setScreen] = useState<ScreenId>("dashboard");
  const Screen = SCREENS[screen];

  return (
    // Blueprint grid shell: compartments separated by real 1px lines (gap-px over edge fill).
    <div className="flex h-screen flex-col gap-px bg-edge text-ink">
      <Titlebar />
      <div className="flex min-h-0 flex-1 gap-px">
        <Sidebar active={screen} onNavigate={setScreen} />
        <main className="relative min-w-0 flex-1 bg-bg">
          <CornerMarks />
          <div className="h-full overflow-y-auto px-10 py-8">
            <Screen onNavigate={setScreen} />
          </div>
        </main>
      </div>
    </div>
  );
}
