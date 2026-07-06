import { MoveRight } from "lucide-react";
import type { PendingChange } from "@sheet-port/shared";
import {
  isFieldChanged,
  parseAppendDiff,
  parseUpdateDiff,
  type AppendDiff,
  type UpdateDiffEntry
} from "../../lib/diff.js";
import { formatValue } from "../../lib/format.js";

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
    <div className="overflow-x-auto rounded-md border border-edge">
      <table className="w-full border-collapse text-[13px]">
        <thead>
          <tr className="bg-raised/60 text-left">
            {columns.map((column) => (
              <th key={column} className="px-3 py-1.5 text-[11px] font-semibold uppercase tracking-widest text-ink-muted">
                {column}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {diff.after.map((record, index) => (
            <tr key={index} className="border-t border-edge bg-accent/10">
              {columns.map((column) => (
                <td key={column} className="px-3 py-1.5 font-mono text-accent">
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
    <div className="overflow-x-auto rounded-md border border-edge">
      <p className="border-b border-edge bg-raised/60 px-3 py-1.5 font-mono text-[11px] text-ink-muted">
        {entry.recordId}
      </p>
      <table className="w-full border-collapse text-[13px]">
        <tbody>
          {fields.map((field) => {
            const changed = isFieldChanged(entry, field);
            return (
              <tr key={field} className="border-t border-edge first:border-t-0">
                <td className="w-32 px-3 py-1.5 text-[11px] font-semibold uppercase tracking-widest text-ink-muted">
                  {field}
                </td>
                <td className={`px-3 py-1.5 font-mono ${changed ? "bg-danger/10 text-danger" : "text-ink-muted"}`}>
                  {formatValue(entry.before?.[field])}
                </td>
                <td className="w-8 px-1 text-center text-ink-muted">
                  <MoveRight size={12} aria-hidden className="inline" />
                </td>
                <td className={`px-3 py-1.5 font-mono ${changed ? "bg-accent/10 text-accent" : "text-ink-muted"}`}>
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
    <pre className="overflow-x-auto rounded-md border border-edge bg-bg p-3 font-mono text-xs text-ink-muted">
      {JSON.stringify(change.diff, null, 2)}
    </pre>
  );
}
