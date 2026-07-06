import type { ConfirmationAction } from "@sheet-port/shared";
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
  label: string;
  description: string;
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
    label: "Read Only",
    description: "Agents can read records but cannot write, update, or delete.",
    read: true,
    write: false,
    deleteRecords: false,
    requireConfirmationFor: [],
    autoApprove: false
  },
  {
    id: "ask",
    label: "Ask Permissions",
    description: "Agents can write, but appends, updates, and deletes wait for your approval.",
    read: true,
    write: true,
    deleteRecords: false,
    requireConfirmationFor: ASK_CONFIRMATIONS,
    autoApprove: false
  },
  {
    id: "auto_approve",
    label: "Auto Approve",
    description: "Agents write without asking. Deletes stay blocked. Enables global auto-approve.",
    read: true,
    write: true,
    deleteRecords: false,
    requireConfirmationFor: [],
    autoApprove: true
  },
  {
    id: "bypass",
    label: "Bypass Permission",
    description: "Full access including deletes, with no approval gate. Enables global auto-approve.",
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
