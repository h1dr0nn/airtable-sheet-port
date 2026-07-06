import { Switch } from "@sheet-port/ui";
import type { DataSource } from "@sheet-port/shared";
import { useSavePermissionRule } from "../../hooks/usePermissions.js";
import type { PermissionRuleRow, SavePermissionRule } from "../../lib/ipc.js";
import { RelativeTime } from "../RelativeTime.js";
import { ConfirmationChips } from "./ConfirmationChips.js";

type ToggleField = "read" | "write" | "deleteRecords";

const TOGGLES: ReadonlyArray<{ field: ToggleField; label: string }> = [
  { field: "read", label: "Read" },
  { field: "write", label: "Write" },
  { field: "deleteRecords", label: "Delete" }
];

function buildTogglePatch(field: ToggleField, checked: boolean): Partial<SavePermissionRule> {
  switch (field) {
    case "read":
      return { read: checked };
    case "write":
      return { write: checked };
    case "deleteRecords":
      return { deleteRecords: checked };
  }
}

/** A source with no rule yet starts fully denied; first toggle creates the row. */
function toSaveShape(source: DataSource, rule: PermissionRuleRow | undefined): SavePermissionRule {
  if (rule) {
    return {
      id: rule.id,
      sourceId: rule.sourceId,
      tableId: rule.tableId,
      read: rule.read,
      write: rule.write,
      deleteRecords: rule.deleteRecords,
      requireConfirmationFor: [...rule.requireConfirmationFor]
    };
  }
  return {
    id: null,
    sourceId: source.id,
    tableId: null,
    read: false,
    write: false,
    deleteRecords: false,
    requireConfirmationFor: []
  };
}

type RuleRowProps = {
  source: DataSource;
  /** The source-wide rule (tableId === null), if one exists yet. */
  rule: PermissionRuleRow | undefined;
};

/** One row per source managing its source-wide rule: access switches plus
 * confirmation chips. Saving is optimistic for existing rules. */
export function RuleRow({ source, rule }: RuleRowProps) {
  const save = useSavePermissionRule();

  const saveWith = (patch: Partial<SavePermissionRule>) => {
    save.mutate({ ...toSaveShape(source, rule), ...patch });
  };

  return (
    <article className="py-4 first:pt-0 last:pb-0">
      <header className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        <span className="text-[13px] font-medium text-ink">{source.name}</span>
        <span className="font-mono text-[11.5px] text-ink-faint">{source.id}/*</span>
        {rule ? (
          <span className="ml-auto text-[12px] text-ink-muted">
            Updated <RelativeTime iso={rule.updatedAt} />
          </span>
        ) : null}
      </header>

      <div className="mt-3 flex flex-wrap items-center gap-6">
        {TOGGLES.map(({ field, label }) => (
          <label key={field} className="flex cursor-pointer items-center gap-2 text-[13px] text-ink">
            <Switch
              checked={rule?.[field] ?? false}
              disabled={save.isPending}
              onCheckedChange={(checked) => saveWith(buildTogglePatch(field, checked))}
              aria-label={`${label} access for ${source.name}`}
            />
            {label}
          </label>
        ))}
      </div>

      <div className="mt-3.5">
        <p className="overline-label mb-2">Require confirmation for</p>
        <ConfirmationChips
          value={rule?.requireConfirmationFor ?? []}
          disabled={save.isPending}
          onChange={(next) => saveWith({ requireConfirmationFor: next })}
        />
      </div>
    </article>
  );
}
