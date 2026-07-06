import { useEffect, useState } from "react";
import {
  Button,
  cn,
  EmptyState,
  FOCUS_RING,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Skeleton
} from "@sheet-port/ui";
import { useSources } from "../hooks/useSources.js";
import { useTablePage, useTableSchema, useTables } from "../hooks/useTables.js";
import type { ScreenId } from "../lib/nav.js";
import { RecordsTable } from "../components/tables/RecordsTable.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

function TableListSkeleton() {
  return (
    <div className="space-y-3">
      <Skeleton className="h-9 w-64" />
      <Skeleton className="h-64 rounded-card" />
    </div>
  );
}

export function Tables({ onNavigate }: { onNavigate: (screen: ScreenId) => void }) {
  const { data: sources, isPending: sourcesPending } = useSources();
  const [sourceId, setSourceId] = useState<string | null>(null);
  const [tableId, setTableId] = useState<string | null>(null);
  const [pageIndex, setPageIndex] = useState(0);

  const sourceList = sources ?? [];
  const effectiveSourceId = sourceId ?? sources?.[0]?.id ?? null;
  const { data: tables, isPending: tablesPending } = useTables(effectiveSourceId);
  const effectiveTableId = tableId ?? tables?.[0]?.tableId ?? null;
  const { data: schema } = useTableSchema(effectiveSourceId, effectiveTableId);
  const { data: page, isPending: pagePending } = useTablePage(effectiveSourceId, effectiveTableId, pageIndex);

  // Reset the table/page selection when the upstream source changes.
  useEffect(() => {
    setTableId(null);
    setPageIndex(0);
  }, [effectiveSourceId]);

  return (
    <>
      <ScreenHeader
        title="Tables"
        description="Browse records through the same read path agents use"
        actions={
          sourceList.length > 0 ? (
            <Select value={effectiveSourceId ?? ""} onValueChange={(next) => setSourceId(next)}>
              <SelectTrigger className="w-56" aria-label="Data source">
                <SelectValue placeholder="Choose a source" />
              </SelectTrigger>
              <SelectContent>
                {sourceList.map((source) => (
                  <SelectItem key={source.id} value={source.id}>
                    {source.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          ) : undefined
        }
      />

      {sourcesPending ? (
        <TableListSkeleton />
      ) : sourceList.length === 0 ? (
        <EmptyState
          title="No Data Sources"
          description="Connect a data source to browse its tables here"
          action={
            <Button size="sm" onClick={() => onNavigate("sources")}>
              Connect a Data Source
            </Button>
          }
        />
      ) : tablesPending ? (
        <TableListSkeleton />
      ) : (tables ?? []).length === 0 ? (
        <EmptyState
          title="No Tables"
          description="This source has not exposed any tables yet"
        />
      ) : (
        <div className="space-y-4">
          <div className="flex flex-wrap gap-1.5" role="tablist" aria-label="Tables">
            {(tables ?? []).map((table) => {
              const isActive = table.tableId === effectiveTableId;
              return (
                <button
                  key={table.tableId}
                  type="button"
                  role="tab"
                  aria-selected={isActive}
                  onClick={() => {
                    setTableId(table.tableId);
                    setPageIndex(0);
                  }}
                  className={cn(
                    "h-8 rounded-lg border px-3 text-[13px] font-medium transition-colors",
                    FOCUS_RING,
                    isActive
                      ? "border-accent/40 bg-accent/[0.08] text-accent"
                      : "border-edge text-ink-muted hover:bg-surface hover:text-ink"
                  )}
                >
                  {table.name}
                  <span className="ml-1.5 font-mono text-[11px] opacity-60">{table.tableId}</span>
                </button>
              );
            })}
          </div>

          {pagePending || !schema || !page ? (
            <Skeleton className="h-96 rounded-card" />
          ) : page.total === 0 ? (
            <EmptyState
              title="No Records"
              description="This table is empty. Agent appends will show up here after commit"
            />
          ) : (
            <RecordsTable schema={schema} page={page} pageIndex={pageIndex} onPageChange={setPageIndex} />
          )}
        </div>
      )}
    </>
  );
}
