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
import { useTranslation } from "../i18n/useTranslation.js";
import { ipc } from "../lib/ipc.js";
import { NAV, type ScreenId } from "../lib/nav.js";
import { queryKeys } from "../lib/queryKeys.js";
import type { ThemeSetting } from "../lib/theme.js";

import type { TranslationKey } from "../i18n/translations.js";

const THEME_ACTIONS: ReadonlyArray<{ value: ThemeSetting; labelKey: TranslationKey }> = [
  { value: "light", labelKey: "palette.themeLight" },
  { value: "dark", labelKey: "palette.themeDark" },
  { value: "system", labelKey: "palette.themeSystem" }
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
  const { t } = useTranslation();
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
        <CommandInput placeholder={t("palette.placeholder")} />
        <CommandList>
          <CommandEmpty>{t("palette.noResults")}</CommandEmpty>
          <CommandGroup heading={t("palette.screens")}>
            {NAV.map((item) => {
              const label = t(item.labelKey);
              return (
                <CommandItem
                  key={item.id}
                  value={`screen-${item.id}`}
                  keywords={[label]}
                  onSelect={() => runAndClose(() => onNavigate(item.screen))}
                >
                  <span className="min-w-0 flex-1 truncate">{label}</span>
                  <span className="shrink-0 text-[11px] text-ink-faint">{t("palette.screen")}</span>
                </CommandItem>
              );
            })}
          </CommandGroup>
          {tables.length > 0 ? (
            <CommandGroup heading={t("palette.tables")}>
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
          <CommandGroup heading={t("palette.actions")}>
            {THEME_ACTIONS.map((action) => {
              const label = t(action.labelKey);
              return (
                <CommandItem
                  key={action.value}
                  value={`theme-${action.value}`}
                  keywords={[label]}
                  onSelect={() => runAndClose(() => setSetting(action.value))}
                >
                  {label}
                </CommandItem>
              );
            })}
            <CommandItem
              value="connect-google-sheets"
              keywords={[t("palette.connectGoogleSheets")]}
              onSelect={() => runAndClose(() => onNavigate("sources"))}
            >
              {t("palette.connectGoogleSheets")}
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    </CommandDialog>
  );
}
