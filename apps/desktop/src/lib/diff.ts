// Runtime guards for the change diff shapes documented in
// crates/sheet-port-core/sql/schema.sql (append -> {"after": records},
// update -> [{"recordId","before","after"}], format -> a FormatPlan).

import type { CellFormat, ColumnWidth, FormatPlan } from "@sheet-port/shared";

export type AppendDiff = {
  after: Array<Record<string, unknown>>;
};

export type UpdateDiffEntry = {
  recordId: string;
  before: Record<string, unknown> | null;
  after: Record<string, unknown>;
};

/** Normalized shape of a format change diff for rendering. */
export type FormatDiff = {
  formats: CellFormat[];
  freezeRows?: number;
  freezeColumns?: number;
  columnWidths: ColumnWidth[];
};

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function parseAppendDiff(diff: unknown): AppendDiff | null {
  if (!isPlainObject(diff) || !Array.isArray(diff.after)) {
    return null;
  }
  const after = diff.after.filter(isPlainObject);
  return after.length === diff.after.length ? { after } : null;
}

export function parseUpdateDiff(diff: unknown): UpdateDiffEntry[] | null {
  if (!Array.isArray(diff)) {
    return null;
  }
  const entries: UpdateDiffEntry[] = [];
  for (const item of diff) {
    if (!isPlainObject(item) || typeof item.recordId !== "string" || !isPlainObject(item.after)) {
      return null;
    }
    const before = isPlainObject(item.before) ? item.before : null;
    entries.push({ recordId: item.recordId, before, after: item.after });
  }
  return entries;
}

/** True when a field value changed between before and after snapshots. */
export function isFieldChanged(entry: UpdateDiffEntry, field: string): boolean {
  if (entry.before === null) {
    return true;
  }
  return JSON.stringify(entry.before[field]) !== JSON.stringify(entry.after[field]);
}

function isCellFormat(value: unknown): value is CellFormat {
  return isPlainObject(value) && typeof value.range === "string";
}

function isColumnWidth(value: unknown): value is ColumnWidth {
  return isPlainObject(value) && typeof value.column === "string" && typeof value.pixels === "number";
}

/**
 * Parses a format change diff (a FormatPlan). Returns null for a shape that
 * carries no formatting at all so the viewer can fall back to raw JSON.
 */
export function parseFormatDiff(diff: unknown): FormatDiff | null {
  if (!isPlainObject(diff)) {
    return null;
  }
  const plan = diff as FormatPlan;
  const formats = Array.isArray(plan.formats) ? plan.formats.filter(isCellFormat) : [];
  const columnWidths = Array.isArray(plan.columnWidths)
    ? plan.columnWidths.filter(isColumnWidth)
    : [];
  const freezeRows = typeof plan.freezeRows === "number" ? plan.freezeRows : undefined;
  const freezeColumns = typeof plan.freezeColumns === "number" ? plan.freezeColumns : undefined;
  const isEmpty =
    formats.length === 0 &&
    columnWidths.length === 0 &&
    freezeRows === undefined &&
    freezeColumns === undefined;
  return isEmpty ? null : { formats, freezeRows, freezeColumns, columnWidths };
}
