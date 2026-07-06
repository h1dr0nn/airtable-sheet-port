import { useEffect, useMemo } from "react";
import { useQueries } from "@tanstack/react-query";
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList
} from "@sheet-port/ui";
import { useSources } from "../hooks/useSources.js";
import { useTheme } from "../hooks/useTheme.js";
import { ipc } from "../lib/ipc.js";
import { NAV, type ScreenId } from "../lib/nav.js";
import { queryKeys } from "../lib/queryKeys.js";
import type { ThemeSetting } from "../lib/theme.js";

const THEME_ACTIONS: ReadonlyArray<{ value: ThemeSetting; label: string }> = [
  { value: "light", label: "Theme: Light" },
  { value: "dark", label: "Theme: Dark" },
  { value: "system", label: "Theme: System" }
];

/** True when some other modal dialog already owns the screen. Radix mounts
 * dialog content only while open, so a simple DOM probe is reliable. */
function isAnotherDialogOpen(): boolean {
  return document.querySelector('[role="dialog"], [role="alertdialog"]') !== null;
}

type TableItem = {
  key: string;
  label: string;
  /** Owning source name, shown right-aligned. */
  hint: string;
};

type CommandPaletteProps = {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onNavigate: (screen: ScreenId) => void;
};

/** Centered command palette (cmdk): screens, tables, and quick actions.
 * Opened from the titlebar search button or Ctrl/Cmd+K anywhere. */
export function CommandPalette({ open, onOpenChange, onNavigate }: CommandPaletteProps) {
  const { setSetting } = useTheme();
  const { data: sources } = useSources();
  const sourceList = sources ?? [];
  const tableQueries = useQueries({
    queries: sourceList.map((source) => ({
      queryKey: queryKeys.tables(source.id),
      queryFn: () => ipc.listTables(source.id),
      enabled: open
    }))
  });

  const tables = useMemo<TableItem[]>(
    () =>
      tableQueries.flatMap((result, index) => {
        const source = sourceList[index];
        if (!source || !result.data) {
          return [];
        }
        return result.data.map(
          (table): TableItem => ({
            key: `table-${table.sourceId}-${table.tableId}`,
            label: table.name,
            hint: source.name
          })
        );
      }),
    [tableQueries, sourceList]
  );

  // Single app-wide Ctrl/Cmd+K shortcut; the palette is mounted exactly once
  // from App, so exactly one listener exists.
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() !== "k" || !(event.ctrlKey || event.metaKey)) {
        return;
      }
      event.preventDefault();
      if (open) {
        onOpenChange(false);
        return;
      }
      if (isAnotherDialogOpen()) {
        return;
      }
      onOpenChange(true);
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [open, onOpenChange]);

  const runAndClose = (run: () => void) => {
    run();
    onOpenChange(false);
  };

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange} title="Command Palette">
      <Command>
        <CommandInput placeholder="Type a command or search..." />
        <CommandList>
          <CommandEmpty>No results found</CommandEmpty>
          <CommandGroup heading="Screens">
            {NAV.map((item) => (
              <CommandItem
                key={item.id}
                value={`screen-${item.id}`}
                keywords={[item.label]}
                onSelect={() => runAndClose(() => onNavigate(item.screen))}
              >
                <span className="min-w-0 flex-1 truncate">{item.label}</span>
                <span className="shrink-0 text-[11px] text-ink-faint">Screen</span>
              </CommandItem>
            ))}
          </CommandGroup>
          {tables.length > 0 ? (
            <CommandGroup heading="Tables">
              {tables.map((table) => (
                <CommandItem
                  key={table.key}
                  value={table.key}
                  keywords={[table.label]}
                  onSelect={() => runAndClose(() => onNavigate("tables"))}
                >
                  <span className="min-w-0 flex-1 truncate">{table.label}</span>
                  <span className="shrink-0 text-[11px] text-ink-faint">{table.hint}</span>
                </CommandItem>
              ))}
            </CommandGroup>
          ) : null}
          <CommandGroup heading="Actions">
            {THEME_ACTIONS.map((action) => (
              <CommandItem
                key={action.value}
                value={`theme-${action.value}`}
                keywords={[action.label]}
                onSelect={() => runAndClose(() => setSetting(action.value))}
              >
                {action.label}
              </CommandItem>
            ))}
            <CommandItem
              value="connect-google-sheets"
              keywords={["Connect Google Sheets"]}
              onSelect={() => runAndClose(() => onNavigate("sources"))}
            >
              Connect Google Sheets
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    </CommandDialog>
  );
}
