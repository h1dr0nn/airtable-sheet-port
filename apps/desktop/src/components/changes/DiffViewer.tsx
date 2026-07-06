import type { PendingChange } from "@sheet-port/shared";
import {
  isFieldChanged,
  parseAppendDiff,
  parseUpdateDiff,
  type AppendDiff,
  type UpdateDiffEntry
} from "../../lib/diff.js";
import { formatValue } from "../../lib/format.js";

const HEADER_CELL_CLASS =
  "border-b border-r border-edge px-3 py-1.5 text-left font-mono text-[10px] font-bold uppercase tracking-[0.08em] text-ink-muted last:border-r-0";
const CELL_CLASS = "border-b border-r border-edge px-3 py-1.5 font-mono text-xs last:border-r-0";

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
    <div className="overflow-x-auto border border-edge">
      <table className="w-full border-collapse">
        <thead>
          <tr>
            <th className={`${HEADER_CELL_CLASS} w-6`} aria-label="Row marker" />
            {columns.map((column) => (
              <th key={column} className={HEADER_CELL_CLASS}>
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {diff.after.map((record, index) => (
            <tr key={index} className="[&:last-child>td]:border-b-0">
              {/* Appended rows are prefixed "+" in phosphor; green stays reserved. */}
              <td className={`${CELL_CLASS} text-center font-bold text-ink`} aria-label="Appended row">
                +
              </td>
              {columns.map((column) => (
                <td key={column} className={`${CELL_CLASS} text-ink`}>
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
    <div className="overflow-x-auto border border-edge">
      <p className="border-b border-edge bg-raised px-3 py-1.5 font-mono text-[10px] uppercase tracking-[0.08em] text-ink-muted">
        REC / {entry.recordId}
      </p>
      <table className="w-full border-collapse">
        <tbody>
          {fields.map((field) => {
            const changed = isFieldChanged(entry, field);
            return (
              <tr key={field} className="[&:last-child>td]:border-b-0">
                <td className={`${CELL_CLASS} w-6 text-center font-bold text-hazard`}>
                  {changed ? ">>" : ""}
                </td>
                <td
                  className={`${CELL_CLASS} w-32 text-[10px] font-bold uppercase tracking-[0.08em] text-ink-muted`}
                >
                  {field}
                </td>
                <td
                  className={`${CELL_CLASS} ${
                    changed ? "text-ink-muted line-through decoration-hazard decoration-1" : "text-ink-muted"
                  }`}
                >
                  {formatValue(entry.before?.[field])}
                </td>
                <td className={`${CELL_CLASS} ${changed ? "font-bold text-ink" : "text-ink-muted"}`}>
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
    <pre className="overflow-x-auto border border-edge bg-bg p-3 font-mono text-[11px] leading-4 text-ink-muted">
      {JSON.stringify(change.diff, null, 2)}
    </pre>
  );
}
