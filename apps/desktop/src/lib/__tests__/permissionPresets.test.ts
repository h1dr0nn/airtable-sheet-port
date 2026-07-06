import { describe, expect, it } from "vitest";
import type { PermissionRuleRow } from "../ipc.js";
import { derivePreset, getPreset, PERMISSION_PRESETS } from "../permissionPresets.js";

/** Builds a rule row matching a preset id, for derive round-trip checks. */
function ruleFromPreset(id: Parameters<typeof getPreset>[0]): PermissionRuleRow {
  const preset = getPreset(id);
  return {
    id: 1,
    sourceId: "google-sheets",
    tableId: null,
    read: preset.read,
    write: preset.write,
    deleteRecords: preset.deleteRecords,
    requireConfirmationFor: [...preset.requireConfirmationFor],
    updatedAt: new Date().toISOString()
  };
}

describe("permission presets", () => {
  it("round-trips every preset through derivePreset", () => {
    for (const preset of PERMISSION_PRESETS) {
      const rule = ruleFromPreset(preset.id);
      expect(derivePreset(rule, preset.autoApprove)).toBe(preset.id);
    }
  });

  it("returns null for a missing rule (fully denied)", () => {
    expect(derivePreset(undefined, false)).toBeNull();
  });

  it("returns null when the rule matches but auto-approve does not", () => {
    // Ask requires autoApprove=false; passing true breaks the match.
    const rule = ruleFromPreset("ask");
    expect(derivePreset(rule, true)).toBeNull();
  });

  it("matches confirmation actions order-independently", () => {
    const rule = ruleFromPreset("ask");
    rule.requireConfirmationFor = [...rule.requireConfirmationFor].reverse();
    expect(derivePreset(rule, false)).toBe("ask");
  });

  it("distinguishes Auto Approve from Bypass by deleteRecords", () => {
    const autoApprove = ruleFromPreset("auto_approve");
    const bypass = ruleFromPreset("bypass");
    expect(derivePreset(autoApprove, true)).toBe("auto_approve");
    expect(derivePreset(bypass, true)).toBe("bypass");
  });
});
