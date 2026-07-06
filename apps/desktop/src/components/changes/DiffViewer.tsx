import { cn } from "@sheet-port/ui";
import type { PendingChange } from "@sheet-port/shared";
import {
  isFieldChanged,
  parseAppendDiff,
  parseUpdateDiff,
  type AppendDiff,
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
  const fields = Object.keys(entry.after);
  return (
    <div className="overflow-x-auto rounded-lg border border-edge">
      <p className="border-b border-edge bg-surface px-3 py-2 text-[11px] font-medium text-ink-muted">
        Record <span className="font-mono text-ink">{entry.recordId}</span>
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
  return (
    <pre className="overflow-x-auto rounded-lg border border-edge bg-surface p-3 font-mono text-[12px] leading-5 text-ink-muted">
      {JSON.stringify(change.diff, null, 2)}
    </pre>
  );
}
