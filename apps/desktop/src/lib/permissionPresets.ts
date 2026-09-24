import type { TranslationKey } from "../i18n/translations.js";
import type { PermissionRuleRow } from "./ipc.js";

/**
 * Named permission presets replace the raw read/write/delete switches. Each
 * preset encodes a source-wide rule, so the UI can offer a small set of
 * intent-level choices instead of low-level flags. Agent writes apply directly
 * once the rule allows them; there is no approval gate.
 */
export type PermissionPresetId = "read_only" | "read_write" | "bypass";

export type PermissionPreset = {
  id: PermissionPresetId;
  /** Translation key for the preset display label. */
  labelKey: TranslationKey;
  /** Translation key for the preset description. */
  descriptionKey: TranslationKey;
  read: boolean;
  write: boolean;
  deleteRecords: boolean;
  /** Selecting a destructive preset warns before applying. */
  requiresConfirmation?: boolean;
};

/** Ordered for the dropdown, least to most permissive. */
export const PERMISSION_PRESETS: readonly PermissionPreset[] = [
  {
    id: "read_only",
    labelKey: "preset.readOnly.label",
    descriptionKey: "preset.readOnly.description",
    read: true,
    write: false,
    deleteRecords: false
  },
  {
    id: "read_write",
    labelKey: "preset.readWrite.label",
    descriptionKey: "preset.readWrite.description",
    read: true,
    write: true,
    deleteRecords: false
  },
  {
    id: "bypass",
    labelKey: "preset.bypass.label",
    descriptionKey: "preset.bypass.description",
    read: true,
    write: true,
    deleteRecords: true,
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

/**
 * Derives which preset a source currently matches from its rule. Returns null
 * when the stored rule does not correspond to any named preset (e.g. a legacy
 * custom combination), so the UI can show an explicit "Custom" placeholder
 * rather than mislabeling it.
 */
export function derivePreset(rule: PermissionRuleRow | undefined): PermissionPresetId | null {
  // No rule yet means fully denied; that matches no preset (all grant read).
  if (!rule) {
    return null;
  }
  const match = PERMISSION_PRESETS.find(
    (preset) =>
      preset.read === rule.read &&
      preset.write === rule.write &&
      preset.deleteRecords === rule.deleteRecords
  );
  return match?.id ?? null;
}
