import type { ConfirmationAction } from "@sheet-port/shared";
import type { TranslationKey } from "../i18n/translations.js";
import type { PermissionRuleRow } from "./ipc.js";

/**
 * Named permission presets replace the raw read/write/delete switches. Each
 * preset encodes a source-wide rule plus whether global auto-approve is on, so
 * the UI can offer a small set of intent-level choices instead of low-level
 * flags. Auto-approve is a GLOBAL backend setting (not per-source): selecting
 * "Auto Approve" or "Bypass Permission" on any source turns it on app-wide.
 */
export type PermissionPresetId =
  | "ask"
  | "read_only"
  | "auto_approve"
  | "bypass";

export type PermissionPreset = {
  id: PermissionPresetId;
  /** Translation key for the preset display label. */
  labelKey: TranslationKey;
  /** Translation key for the preset description. */
  descriptionKey: TranslationKey;
  read: boolean;
  write: boolean;
  deleteRecords: boolean;
  requireConfirmationFor: readonly ConfirmationAction[];
  autoApprove: boolean;
  /** Selecting a destructive preset warns before applying. */
  requiresConfirmation?: boolean;
};

const ASK_CONFIRMATIONS: readonly ConfirmationAction[] = [
  "append",
  "update",
  "delete",
  "bulk_update"
];

/** Ordered for the dropdown, least to most permissive. */
export const PERMISSION_PRESETS: readonly PermissionPreset[] = [
  {
    id: "read_only",
    labelKey: "preset.readOnly.label",
    descriptionKey: "preset.readOnly.description",
    read: true,
    write: false,
    deleteRecords: false,
    requireConfirmationFor: [],
    autoApprove: false
  },
  {
    id: "ask",
    labelKey: "preset.ask.label",
    descriptionKey: "preset.ask.description",
    read: true,
    write: true,
    deleteRecords: false,
    requireConfirmationFor: ASK_CONFIRMATIONS,
    autoApprove: false
  },
  {
    id: "auto_approve",
    labelKey: "preset.autoApprove.label",
    descriptionKey: "preset.autoApprove.description",
    read: true,
    write: true,
    deleteRecords: false,
    requireConfirmationFor: [],
    autoApprove: true
  },
  {
    id: "bypass",
    labelKey: "preset.bypass.label",
    descriptionKey: "preset.bypass.description",
    read: true,
    write: true,
    deleteRecords: true,
    requireConfirmationFor: [],
    autoApprove: true,
    requiresConfirmation: true
  }
];

export function getPreset(id: PermissionPresetId): PermissionPreset {
  const preset = PERMISSION_PRESETS.find((item) => item.id === id);
  if (!preset) {
    throw new Error(`Unknown permission preset ${id}`);
  }
  return preset;
}

/** Compares two confirmation lists order-independently. */
function sameConfirmations(a: readonly string[], b: readonly string[]): boolean {
  if (a.length !== b.length) {
    return false;
  }
  const set = new Set(a);
  return b.every((item) => set.has(item));
}

/**
 * Derives which preset a source currently matches from its rule and the global
 * auto-approve flag. Returns null when the stored rule does not correspond to
 * any named preset (e.g. a legacy custom combination), so the UI can show an
 * explicit "Custom" placeholder rather than mislabeling it.
 */
export function derivePreset(
  rule: PermissionRuleRow | undefined,
  autoApproveWrites: boolean
): PermissionPresetId | null {
  // No rule yet means fully denied; that matches no preset (all grant read).
  if (!rule) {
    return null;
  }
  const match = PERMISSION_PRESETS.find(
    (preset) =>
      preset.read === rule.read &&
      preset.write === rule.write &&
      preset.deleteRecords === rule.deleteRecords &&
      preset.autoApprove === autoApproveWrites &&
      sameConfirmations(preset.requireConfirmationFor, rule.requireConfirmationFor)
  );
  return match?.id ?? null;
}
