import "@glideapps/glide-data-grid/dist/index.css";
import {
  DataEditor,
  GridCellKind,
  type EditableGridCell,
  type GridCell,
  type GridColumn,
  type Item
} from "@glideapps/glide-data-grid";
import { useCallback, useMemo, useState } from "react";
import type { GridData } from "../../lib/ipc.js";
import { useGlideTheme } from "./glideTheme.js";

const DEFAULT_COLUMN_WIDTH = 170;
const MIN_COLUMN_WIDTH = 80;
// Heuristic starting width so columns aren't uniformly narrow before any resize.
const PX_PER_CHAR = 8.5;
const COLUMN_PADDING_PX = 28;

type SheetGridProps = {
  grid: GridData;
  /** In-sheet find query; rows are filtered to those containing it. */
  query: string;
  /** Called with the ORIGINAL (unfiltered) row index of the edited cell. */
  onEditCell: (rowIndex: number, columnId: string, value: string) => void;
};

/** Estimates a readable starting width from the column title length. */
function estimateWidth(title: string): number {
  return Math.min(320, Math.max(DEFAULT_COLUMN_WIDTH, title.length * PX_PER_CHAR + COLUMN_PADDING_PX));
}

/**
 * The editable canvas grid. Wraps glide-data-grid: string cells with overlay
 * editing, app-token theming, per-column resize, and client-side row filtering
 * driven by the toolbar's in-sheet search.
 */
export function SheetGrid({ grid, query, onEditCell }: SheetGridProps) {
  const theme = useGlideTheme();
  const [widths, setWidths] = useState<Record<string, number>>({});

  // Original row indices that survive the in-sheet filter, preserving order so
  // editing a filtered row still targets the correct underlying row.
  const visibleRows = useMemo<number[]>(() => {
    const trimmed = query.trim().toLowerCase();
    if (trimmed === "") {
      return grid.rows.map((_, index) => index);
    }
    return grid.rows.reduce<number[]>((matches, row, index) => {
      const hit = grid.columns.some((column) =>
        (row[column.id] ?? "").toLowerCase().includes(trimmed)
      );
      if (hit) {
        matches.push(index);
      }
      return matches;
    }, []);
  }, [grid, query]);

  const columns = useMemo<GridColumn[]>(
    () =>
      grid.columns.map((column) => ({
        id: column.id,
        title: column.title,
        width: widths[column.id] ?? estimateWidth(column.title)
      })),
    [grid.columns, widths]
  );

  const getCellContent = useCallback(
    (cell: Item): GridCell => {
      const [colIndex, displayRow] = cell;
      const column = grid.columns[colIndex];
      const originalRow = visibleRows[displayRow];
      const value =
        column && originalRow !== undefined ? grid.rows[originalRow]?.[column.id] ?? "" : "";
      return {
        kind: GridCellKind.Text,
        data: value,
        displayData: value,
        allowOverlay: true
      };
    },
    [grid, visibleRows]
  );

  const onCellEdited = useCallback(
    (cell: Item, newValue: EditableGridCell): void => {
      if (newValue.kind !== GridCellKind.Text) {
        return;
      }
      const [colIndex, displayRow] = cell;
      const column = grid.columns[colIndex];
      const originalRow = visibleRows[displayRow];
      if (!column || originalRow === undefined) {
        return;
      }
      onEditCell(originalRow, column.id, newValue.data);
    },
    [grid.columns, visibleRows, onEditCell]
  );

  const onColumnResize = useCallback((column: GridColumn, newSize: number): void => {
    if (column.id === undefined) {
      return;
    }
    const id = column.id;
    setWidths((previous) => ({ ...previous, [id]: Math.max(MIN_COLUMN_WIDTH, newSize) }));
  }, []);

  return (
    <div className="h-full w-full">
      <DataEditor
        theme={theme}
        columns={columns}
        rows={visibleRows.length}
        getCellContent={getCellContent}
        onCellEdited={onCellEdited}
        onColumnResize={onColumnResize}
        rowMarkers="number"
        smoothScrollX
        smoothScrollY
        width="100%"
        height="100%"
        getCellsForSelection
      />
    </div>
  );
}
