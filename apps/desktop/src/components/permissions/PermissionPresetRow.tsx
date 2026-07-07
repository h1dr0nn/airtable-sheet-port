import { useState } from "react";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue
} from "@sheet-port/ui";
import type { DataSource } from "@sheet-port/shared";
import { useSavePermissionRule } from "../../hooks/usePermissions.js";
import { useSetAutoApprove } from "../../hooks/useSettings.js";
import type { PermissionRuleRow, SavePermissionRule } from "../../lib/ipc.js";
import {
  PERMISSION_PRESETS,
  derivePreset,
  getPreset,
  type PermissionPreset,
  type PermissionPresetId
} from "../../lib/permissionPresets.js";
import { ConfirmDialog } from "../ConfirmDialog.js";
import { RelativeTime } from "../RelativeTime.js";

/** Builds the save payload for a preset, reusing the rule id when one exists. */
function toSaveShape(
  source: DataSource,
  rule: PermissionRuleRow | undefined,
  preset: PermissionPreset
): SavePermissionRule {
  return {
    id: rule?.id ?? null,
    sourceId: source.id,
    tableId: null,
    read: preset.read,
    write: preset.write,
    deleteRecords: preset.deleteRecords,
    requireConfirmationFor: [...preset.requireConfirmationFor]
  };
}

type PermissionPresetRowProps = {
  source: DataSource;
  /** The source-wide rule (tableId === null), if one exists yet. */
  rule: PermissionRuleRow | undefined;
  /** Global auto-approve flag, used to derive the active preset. */
  autoApproveWrites: boolean;
};

/**
 * One row per connected source. The source-wide rule is chosen from a small set
 * of named presets instead of individual switches; applying a preset saves the
 * rule AND sets global auto-approve to the preset's value. Destructive presets
 * (Bypass) confirm first.
 */
export function PermissionPresetRow({ source, rule, autoApproveWrites }: PermissionPresetRowProps) {
  const save = useSavePermissionRule();
  const setAutoApprove = useSetAutoApprove();
  const [pendingBypass, setPendingBypass] = useState<PermissionPresetId | null>(null);

  const activePreset = derivePreset(rule, autoApproveWrites);
  const isBusy = save.isPending || setAutoApprove.isPending;

  const applyPreset = (preset: PermissionPreset) => {
    save.mutate(toSaveShape(source, rule, preset));
    // Auto-approve is global; keep it in sync with the chosen preset.
    if (preset.autoApprove !== autoApproveWrites) {
      setAutoApprove.mutate(preset.autoApprove);
    }
  };

  const handleChange = (value: string) => {
    const preset = getPreset(value as PermissionPresetId);
    if (preset.requiresConfirmation) {
      // Confirm destructive presets before applying (security warning modal).
      setPendingBypass(preset.id);
      return;
    }
    applyPreset(preset);
  };

  return (
    <article className="py-4 first:pt-0 last:pb-0">
      <div className="flex items-center justify-between gap-4">
        {/* Source info stretches left; the preset control stays right-aligned. */}
        <div className="min-w-0 flex-1">
          <span className="block truncate text-[13px] font-medium text-ink">{source.name}</span>
          {rule ? (
            <p className="mt-0.5 text-[12px] text-ink-muted">
              Updated <RelativeTime iso={rule.updatedAt} />
            </p>
          ) : null}
        </div>

        <Select value={activePreset ?? undefined} onValueChange={handleChange} disabled={isBusy}>
          <SelectTrigger
            className="w-44 shrink-0"
            aria-label={`Permission preset for ${source.name}`}
          >
            <SelectValue placeholder="Custom" />
          </SelectTrigger>
          <SelectContent>
            {PERMISSION_PRESETS.map((preset) => (
              <SelectItem key={preset.id} value={preset.id}>
                {preset.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      <p className="mt-2 text-[12px] leading-4 text-ink-muted">
        {activePreset
          ? getPreset(activePreset).description
          : "This source uses a custom rule. Pick a preset to normalize it."}
      </p>

      <ConfirmDialog
        open={pendingBypass !== null}
        onOpenChange={(open) => {
          if (!open) {
            setPendingBypass(null);
          }
        }}
        title="Bypass Permission?"
        description="Agents get full access including deletes, with no approval gate, and global auto-approve is turned on. Only choose this if you fully trust every connected agent."
        confirmLabel="Enable Bypass"
        isPending={isBusy}
        onConfirm={() => {
          if (pendingBypass) {
            applyPreset(getPreset(pendingBypass));
          }
          setPendingBypass(null);
        }}
      />
    </article>
  );
}
