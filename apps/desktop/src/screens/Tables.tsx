import { useEffect, useState } from "react";
import {
  EmptyState,
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  Skeleton,
  cn
} from "@sheet-port/ui";
import { useSources } from "../hooks/useSources.js";
import { useTablePage, useTableSchema, useTables } from "../hooks/useTables.js";
import { RecordsTable } from "../components/tables/RecordsTable.js";
import { ScreenHeader } from "../components/ScreenHeader.js";

function TableListSkeleton() {
  return (
    <div className="space-y-3">
      <Skeleton className="h-9 w-64" />
      <Skeleton className="h-64" />
    </div>
  );
}

export function Tables() {
  const { data: sources, isPending: sourcesPending } = useSources();
  const [sourceId, setSourceId] = useState<string | null>(null);
  const [tableId, setTableId] = useState<string | null>(null);
  const [pageIndex, setPageIndex] = useState(0);

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

  const isLoading = sourcesPending || tablesPending;
  const meta = isLoading ? "TBL / SCAN" : `SRC ${(sources ?? []).length} / TBL ${(tables ?? []).length}`;

  return (
    <>
      <ScreenHeader
        title="Tables"
        description="Browse records through the same read path agents use"
        meta={meta}
        actions={
          <Select value={effectiveSourceId ?? ""} onValueChange={(next) => setSourceId(next)}>
            <SelectTrigger className="w-56" aria-label="Data source">
              <SelectValue placeholder="Choose a source" />
            </SelectTrigger>
            <SelectContent>
              {(sources ?? []).map((source) => (
                <SelectItem key={source.id} value={source.id}>
                  {source.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        }
      />

      {isLoading ? (
        <TableListSkeleton />
      ) : (tables ?? []).length === 0 ? (
        <EmptyState
          title="No tables"
          description="Placeholder sources expose their tables once the connector is authenticated"
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
                    "border px-3 py-1.5 font-mono text-[11px] uppercase tracking-[0.05em] transition-colors",
                    "focus-visible:outline focus-visible:outline-1 focus-visible:outline-offset-2 focus-visible:outline-hazard",
                    isActive
                      ? "border-ink bg-ink font-bold text-bg"
                      : "border-edge text-ink-muted hover:border-edge-strong hover:text-ink"
                  )}
                >
                  {table.name}
                  <span className="ml-1.5 text-[10px] opacity-70">{table.tableId}</span>
                </button>
              );
            })}
          </div>

          {pagePending || !schema || !page ? (
            <Skeleton className="h-96" />
          ) : page.total === 0 ? (
            <EmptyState
              title="No records"
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
