import type { ReactNode } from "react";
import { cn } from "@sheet-port/ui";
import type { CellFormat, PendingChange } from "@sheet-port/shared";
import { useTranslation } from "../../i18n/useTranslation.js";
import {
  isFieldChanged,
  parseAppendDiff,
  parseFormatDiff,
  parseUpdateDiff,
  type AppendDiff,
  type FormatDiff,
  type UpdateDiffEntry
} from "../../lib/diff.js";
import { formatValue } from "../../lib/format.js";

const HEADER_CELL_CLASS = "h-8 px-3 text-left text-[11px] font-medium text-ink-muted";
const VALUE_CELL_CLASS = "h-9 px-3 font-mono text-[12.5px]";

function collectColumns(records: Array<Record<string, unknown>>): string[] {
  const columns: string[] = [];
  for (const record of records) {
    for (const key of Object.keys(record)) {
      if (!columns.includes(key)) {
        columns.push(key);
      }
    }
  }
  return columns;
}

function AppendDiffTable({ diff }: { diff: AppendDiff }) {
  const columns = collectColumns(diff.after);
  return (
    <div className="overflow-x-auto rounded-lg border border-edge">
      <table className="w-full border-collapse">
        <thead>
          <tr className="border-b border-edge bg-surface">
            <th className={cn(HEADER_CELL_CLASS, "w-8")} aria-label="Row marker" />
            {columns.map((column) => (
              <th key={column} className={HEADER_CELL_CLASS}>
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {diff.after.map((record, index) => (
            <tr key={index} className="border-b border-edge bg-success/[0.08] last:border-b-0">
              <td className={cn(VALUE_CELL_CLASS, "text-center font-medium text-success")} aria-label="Appended row">
                +
              </td>
              {columns.map((column) => (
                <td key={column} className={cn(VALUE_CELL_CLASS, "text-ink")}>
                  {formatValue(record[column])}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function UpdateEntryTable({ entry }: { entry: UpdateDiffEntry }) {
  const { t } = useTranslation();
  const fields = Object.keys(entry.after);
  return (
    <div className="overflow-x-auto rounded-lg border border-edge">
      <p className="border-b border-edge bg-surface px-3 py-2 text-[11px] font-medium text-ink-muted">
        {t("changes.recordLabel", { id: entry.recordId })}
      </p>
      <table className="w-full border-collapse">
        <tbody>
          {fields.map((field) => {
            const changed = isFieldChanged(entry, field);
            return (
              <tr key={field} className="border-b border-edge last:border-b-0">
                {/* Changed rows carry a 2px accent marker on the left edge. */}
                <td
                  className={cn(
                    "h-9 w-32 border-l-2 px-3 text-[11px] font-medium text-ink-muted",
                    changed ? "border-l-accent" : "border-l-transparent"
                  )}
                >
                  {field}
                </td>
                <td
                  className={cn(
                    VALUE_CELL_CLASS,
                    changed
                      ? "bg-danger/[0.08] text-ink-muted line-through decoration-danger/50"
                      : "text-ink-muted"
                  )}
                >
                  {formatValue(entry.before?.[field])}
                </td>
                <td
                  className={cn(
                    VALUE_CELL_CLASS,
                    changed ? "bg-success/[0.08] font-medium text-ink" : "text-ink-muted"
                  )}
                >
                  {formatValue(entry.after[field])}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

function FormatChip({ children }: { children: ReactNode }) {
  return (
    <span className="inline-flex items-center gap-1 rounded-md border border-edge bg-surface px-2 py-0.5 text-[11.5px] text-ink">
      {children}
    </span>
  );
}

function ColorChip({ label, color }: { label: string; color: string }) {
  return (
    <FormatChip>
      <span
        className="h-3 w-3 rounded-sm border border-edge"
        style={{ backgroundColor: color }}
        aria-hidden
      />
      {label} {color}
    </FormatChip>
  );
}

/** Compact pills for the properties one cell-format op sets. */
function FormatOpChips({ format }: { format: CellFormat }) {
  const chips: ReactNode[] = [];
  if (format.bold) chips.push(<FormatChip key="bold">Bold</FormatChip>);
  if (format.italic) chips.push(<FormatChip key="italic">Italic</FormatChip>);
  if (typeof format.fontSize === "number") {
    chips.push(<FormatChip key="size">Size {format.fontSize}</FormatChip>);
  }
  if (format.horizontalAlignment) {
    chips.push(<FormatChip key="align">Align {format.horizontalAlignment}</FormatChip>);
  }
  if (format.numberFormat) {
    chips.push(<FormatChip key="numberFormat">Format {format.numberFormat}</FormatChip>);
  }
  if (typeof format.wrap === "boolean") {
    chips.push(<FormatChip key="wrap">{format.wrap ? "Wrap" : "No wrap"}</FormatChip>);
  }
  if (format.border) chips.push(<FormatChip key="border">Border {format.border}</FormatChip>);
  if (format.fontColor) {
    chips.push(<ColorChip key="fontColor" label="Text" color={format.fontColor} />);
  }
  if (format.backgroundColor) {
    chips.push(<ColorChip key="backgroundColor" label="Fill" color={format.backgroundColor} />);
  }
  return <div className="flex flex-wrap gap-1.5">{chips}</div>;
}

function FormatDiffView({ diff }: { diff: FormatDiff }) {
  const { t } = useTranslation();
  const hasLayout =
    diff.freezeRows !== undefined ||
    diff.freezeColumns !== undefined ||
    diff.columnWidths.length > 0;
  return (
    <div className="space-y-2">
      {diff.formats.length > 0 ? (
        <div className="overflow-hidden rounded-lg border border-edge">
          <p className="border-b border-edge bg-surface px-3 py-2 text-[11px] font-medium text-ink-muted">
            {t("changes.formatCellsHeading")}
          </p>
          <ul className="divide-y divide-edge">
            {diff.formats.map((format, index) => (
              <li key={index} className="flex flex-col gap-2 px-3 py-2.5">
                <span className="font-mono text-[12px] text-ink">{format.range}</span>
                <FormatOpChips format={format} />
              </li>
            ))}
          </ul>
        </div>
      ) : null}
      {hasLayout ? (
        <div className="overflow-hidden rounded-lg border border-edge">
          <p className="border-b border-edge bg-surface px-3 py-2 text-[11px] font-medium text-ink-muted">
            {t("changes.formatLayoutHeading")}
          </p>
          <ul className="space-y-1 px-3 py-2.5 text-[12.5px] text-ink">
            {diff.freezeRows !== undefined ? (
              <li>{t("changes.formatFreezeRows", { count: diff.freezeRows })}</li>
            ) : null}
            {diff.freezeColumns !== undefined ? (
              <li>{t("changes.formatFreezeColumns", { count: diff.freezeColumns })}</li>
            ) : null}
            {diff.columnWidths.map((width) => (
              <li key={width.column} className="font-mono text-[12px]">
                {t("changes.formatColumnWidth", { column: width.column, pixels: width.pixels })}
              </li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  );
}

/** Renders the agent-visible diff for a pending change; falls back to raw JSON. */
export function DiffViewer({ change }: { change: PendingChange }) {
  if (change.type === "append") {
    const appendDiff = parseAppendDiff(change.diff);
    if (appendDiff) {
      return <AppendDiffTable diff={appendDiff} />;
    }
  }
  if (change.type === "update") {
    const updateDiff = parseUpdateDiff(change.diff);
    if (updateDiff) {
      return (
        <div className="space-y-2">
          {updateDiff.map((entry) => (
            <UpdateEntryTable key={entry.recordId} entry={entry} />
          ))}
        </div>
      );
    }
  }
  if (change.type === "format") {
    const formatDiff = parseFormatDiff(change.diff);
    if (formatDiff) {
      return <FormatDiffView diff={formatDiff} />;
    }
  }
  return (
    <pre className="overflow-x-auto rounded-lg border border-edge bg-surface p-3 font-mono text-[12px] leading-5 text-ink-muted">
      {JSON.stringify(change.diff, null, 2)}
    </pre>
  );
}
