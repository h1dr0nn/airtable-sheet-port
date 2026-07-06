import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { useQueries } from "@tanstack/react-query";
import { cn } from "@sheet-port/ui";
import { useSources } from "../hooks/useSources.js";
import { ipc } from "../lib/ipc.js";
import { NAV, type ScreenId } from "../lib/nav.js";
import { queryKeys } from "../lib/queryKeys.js";

const MAX_RESULTS = 10;

type QuickNavItem = {
  key: string;
  label: string;
  /** Small right-aligned context: "Screen" or the owning source name. */
  hint: string;
  screen: ScreenId;
};

type QuickNavProps = {
  open: boolean;
  onClose: () => void;
  onNavigate: (screen: ScreenId) => void;
};

/** Lightweight quick-nav: filters nav screens and table names, Enter navigates. */
export function QuickNav({ open, onClose, onNavigate }: QuickNavProps) {
  const [query, setQuery] = useState("");
  const [activeIndex, setActiveIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement>(null);

  const { data: sources } = useSources();
  const sourceList = sources ?? [];
  const tableQueries = useQueries({
    queries: sourceList.map((source) => ({
      queryKey: queryKeys.tables(source.id),
      queryFn: () => ipc.listTables(source.id),
      enabled: open
    }))
  });

  const items = useMemo<QuickNavItem[]>(() => {
    const screens = NAV.map(
      (item): QuickNavItem => ({
        key: `screen-${item.id}`,
        label: item.label,
        hint: "Screen",
        screen: item.screen
      })
    );
    const tables = tableQueries.flatMap((result, index) => {
      const source = sourceList[index];
      if (!source || !result.data) {
        return [];
      }
      return result.data.map(
        (table): QuickNavItem => ({
          key: `table-${table.sourceId}-${table.tableId}`,
          label: table.name,
          hint: source.name,
          screen: "tables"
        })
      );
    });
    const all = [...screens, ...tables];
    const needle = query.trim().toLowerCase();
    const filtered =
      needle === "" ? all : all.filter((item) => item.label.toLowerCase().includes(needle));
    return filtered.slice(0, MAX_RESULTS);
  }, [tableQueries, sourceList, query]);

  // Reset per open so a stale query never greets the next search.
  useEffect(() => {
    if (open) {
      setQuery("");
      setActiveIndex(0);
      inputRef.current?.focus();
    }
  }, [open]);

  useEffect(() => {
    setActiveIndex(0);
  }, [query]);

  if (!open) {
    return null;
  }

  const go = (item: QuickNavItem | undefined) => {
    if (!item) {
      return;
    }
    onNavigate(item.screen);
    onClose();
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      setActiveIndex((index) => (items.length === 0 ? 0 : (index + 1) % items.length));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setActiveIndex((index) =>
        items.length === 0 ? 0 : (index - 1 + items.length) % items.length
      );
    } else if (event.key === "Enter") {
      event.preventDefault();
      go(items[activeIndex]);
    }
  };

  return (
    <>
      {/* Click-away catcher under the panel. */}
      <div className="fixed inset-0 z-40" onMouseDown={onClose} />
      <div
        role="dialog"
        aria-label="Quick navigation"
        className={cn(
          "absolute left-0 top-full z-50 mt-1 w-72 overflow-hidden rounded-lg",
          "border border-edge bg-raised shadow-pop motion-safe:animate-fade-in"
        )}
      >
        <input
          ref={inputRef}
          type="text"
          role="combobox"
          aria-expanded={items.length > 0}
          aria-controls="quick-nav-results"
          aria-activedescendant={items[activeIndex] ? `quick-nav-${items[activeIndex].key}` : undefined}
          placeholder="Go to screen or table..."
          value={query}
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={handleKeyDown}
          className={cn(
            "w-full border-b border-edge bg-transparent px-3 py-2.5",
            "font-sans text-[13px] text-ink outline-none placeholder:text-ink-faint"
          )}
        />
        <ul id="quick-nav-results" role="listbox" aria-label="Results" className="max-h-72 overflow-y-auto p-1">
          {items.length === 0 ? (
            <li className="px-2.5 py-2 text-[12.5px] text-ink-muted">No matches</li>
          ) : (
            items.map((item, index) => (
              <li key={item.key} role="option" aria-selected={index === activeIndex} id={`quick-nav-${item.key}`}>
                <button
                  type="button"
                  tabIndex={-1}
                  onMouseEnter={() => setActiveIndex(index)}
                  onClick={() => go(item)}
                  className={cn(
                    "flex w-full items-center justify-between gap-3 rounded-md px-2.5 py-1.5",
                    "text-left text-[13px] text-ink transition-colors",
                    index === activeIndex ? "bg-accent/10" : "hover:bg-surface"
                  )}
                >
                  <span className="min-w-0 truncate">{item.label}</span>
                  <span className="shrink-0 text-[11px] text-ink-faint">{item.hint}</span>
                </button>
              </li>
            ))
          )}
        </ul>
      </div>
    </>
  );
}
