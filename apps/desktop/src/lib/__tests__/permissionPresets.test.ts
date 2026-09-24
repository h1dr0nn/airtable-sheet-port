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
    updatedAt: new Date().toISOString()
  };
}

describe("permission presets", () => {
  it("round-trips every preset through derivePreset", () => {
    for (const preset of PERMISSION_PRESETS) {
      expect(derivePreset(ruleFromPreset(preset.id))).toBe(preset.id);
    }
  });

  it("returns null for a missing rule (fully denied)", () => {
    expect(derivePreset(undefined)).toBeNull();
  });

  it("returns null for a combination no preset covers", () => {
    const rule = ruleFromPreset("read_only");
    rule.read = false;
    expect(derivePreset(rule)).toBeNull();
  });

  it("distinguishes Read & Write from Bypass by deleteRecords", () => {
    expect(derivePreset(ruleFromPreset("read_write"))).toBe("read_write");
    expect(derivePreset(ruleFromPreset("bypass"))).toBe("bypass");
  });

  it("only the destructive preset asks for confirmation", () => {
    const confirming = PERMISSION_PRESETS.filter((preset) => preset.requiresConfirmation);
    expect(confirming.map((preset) => preset.id)).toEqual(["bypass"]);
  });
});
