import {
  createColumnHelper,
  flexRender,
  getCoreRowModel,
  useReactTable,
  type RowData
} from "@tanstack/react-table";
import { useMemo, type ReactNode } from "react";
import { Badge, Button, cn } from "@sheet-port/ui";
import type { FieldSchema, TableRecord, TableSchema } from "@sheet-port/shared";
import { TABLE_PAGE_SIZE } from "../../lib/constants.js";
import { formatValue } from "../../lib/format.js";
import type { TablePage } from "../../lib/ipc.js";

// Declaration merging: TanStack Table exposes ColumnMeta for app-defined
// metadata; the type parameters must match the library's declaration.
declare module "@tanstack/react-table" {
  interface ColumnMeta<TData extends RowData, TValue> {
    align?: "left" | "right";
  }
}

function renderTypedCell(field: FieldSchema, value: unknown): ReactNode {
  if (value === null || value === undefined || value === "") {
    return <span className="text-ink-muted">-</span>;
  }
  switch (field.type) {
    case "boolean":
      // ASCII truth markers; green stays reserved for the MCP readout.
      return value === true ? (
        <span className="font-mono text-xs font-bold text-ink" aria-label="true">
          X
        </span>
      ) : (
        <span className="font-mono text-xs text-ink-muted" aria-label="false">
          -
        </span>
      );
    case "email":
      return <span className="font-mono text-xs">{formatValue(value)}</span>;
    case "enum":
      return <Badge variant="muted">{formatValue(value)}</Badge>;
    case "number":
      return <span className="font-mono text-xs tabular-nums">{formatValue(value)}</span>;
    case "date":
      return <span className="font-mono text-xs">{formatValue(value)}</span>;
    default:
      return <span className="font-mono text-xs">{formatValue(value)}</span>;
  }
}

type RecordsTableProps = {
  schema: TableSchema;
  page: TablePage;
  pageIndex: number;
  onPageChange: (page: number) => void;
};

export function RecordsTable({ schema, page, pageIndex, onPageChange }: RecordsTableProps) {
  const columnHelper = createColumnHelper<TableRecord>();
  const columns = useMemo(
    () => [
      columnHelper.accessor("id", {
        id: "__record_id",
        header: "Record",
        cell: (info) => <span className="font-mono text-[11px] text-ink-muted">{info.getValue()}</span>
      }),
      ...schema.fields.map((field) =>
        columnHelper.accessor((row) => row.fields[field.name], {
          id: field.name,
          header: field.name,
          cell: (info) => renderTypedCell(field, info.getValue()),
          meta: { align: field.type === "number" ? "right" : "left" }
        })
      )
    ],
    [columnHelper, schema.fields]
  );

  const table = useReactTable({ data: page.records, columns, getCoreRowModel: getCoreRowModel() });
  const pageCount = Math.max(1, Math.ceil(page.total / TABLE_PAGE_SIZE));
  const firstRow = page.total === 0 ? 0 : pageIndex * TABLE_PAGE_SIZE + 1;
  const lastRow = Math.min(page.total, (pageIndex + 1) * TABLE_PAGE_SIZE);

  return (
    <div className="border border-edge bg-surface">
      <div className="overflow-x-auto">
        <table className="w-full border-collapse">
          <thead>
            {table.getHeaderGroups().map((headerGroup) => (
              <tr key={headerGroup.id}>
                {headerGroup.headers.map((header) => {
                  const align = header.column.columnDef.meta?.align;
                  return (
                    <th
                      key={header.id}
                      className={cn(
                        "h-8 border-b border-r border-edge bg-raised px-3 text-left align-middle",
                        "font-mono text-[10px] font-bold uppercase tracking-[0.08em] text-ink-muted",
                        "last:border-r-0",
                        align === "right" && "text-right"
                      )}
                    >
                      {flexRender(header.column.columnDef.header, header.getContext())}
                    </th>
                  );
                })}
              </tr>
            ))}
          </thead>
          <tbody>
            {table.getRowModel().rows.map((row) => (
              <tr key={row.id} className="transition-colors hover:bg-raised [&:last-child>td]:border-b-0">
                {row.getVisibleCells().map((cell) => {
                  const align = cell.column.columnDef.meta?.align;
                  return (
                    <td
                      key={cell.id}
                      className={cn(
                        "h-8 border-b border-r border-edge px-3 align-middle last:border-r-0",
                        align === "right" && "text-right"
                      )}
                    >
                      {flexRender(cell.column.columnDef.cell, cell.getContext())}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <footer className="flex items-center justify-between border-t border-edge px-3 py-2">
        <p className="font-mono text-[11px] uppercase tracking-[0.05em] text-ink-muted">
          <span className="tabular-nums text-ink">
            {firstRow}-{lastRow}
          </span>{" "}
          of <span className="tabular-nums text-ink">{page.total}</span> records
        </p>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            disabled={pageIndex === 0}
            onClick={() => onPageChange(pageIndex - 1)}
          >
            {"< Prev"}
          </Button>
          <span className="font-mono text-[11px] tabular-nums text-ink-muted">
            {pageIndex + 1}/{pageCount}
          </span>
          <Button
            variant="outline"
            size="sm"
            disabled={pageIndex + 1 >= pageCount}
            onClick={() => onPageChange(pageIndex + 1)}
          >
            {"Next >"}
          </Button>
        </div>
      </footer>
    </div>
  );
}
