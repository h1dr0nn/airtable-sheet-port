import type { PermissionRule } from "@sheet-port/shared";
import { beforeEach, describe, expect, it } from "vitest";
import { PermissionDeniedError, PermissionService } from "../src/index.js";
import { InMemoryRuleProvider } from "./fakes.js";

const SOURCE = "src-a";
const TABLE = "t1";

function makeRule(overrides: Partial<PermissionRule> = {}): PermissionRule {
  return {
    sourceId: SOURCE,
    read: true,
    write: true,
    deleteRecords: false,
    requireConfirmationFor: [],
    ...overrides
  };
}

describe("PermissionService", () => {
  let provider: InMemoryRuleProvider;
  let service: PermissionService;

  beforeEach(() => {
    provider = new InMemoryRuleProvider();
    service = new PermissionService(provider);
  });

  describe("read gating", () => {
    it("throws PermissionDeniedError when no rule exists for the source", () => {
      expect(() => service.assertCanRead(SOURCE, TABLE)).toThrow(PermissionDeniedError);
    });

    it("throws when the matching rule disables read", () => {
      provider.set(makeRule({ read: false }));
      expect(() => service.assertCanRead(SOURCE, TABLE)).toThrow(`Read access denied for ${SOURCE}/${TABLE}`);
    });

    it("allows read when the matching rule enables read", () => {
      provider.set(makeRule({ read: true }));
      expect(() => service.assertCanRead(SOURCE, TABLE)).not.toThrow();
    });

    it("uses the table-specific rule over the source-wide rule", () => {
      // Arrange
      provider.set(makeRule({ read: true }));
      provider.set(makeRule({ tableId: TABLE, read: false }));

      // Act + Assert
      expect(() => service.assertCanRead(SOURCE, TABLE)).toThrow(PermissionDeniedError);
      expect(() => service.assertCanRead(SOURCE, "other-table")).not.toThrow();
    });
  });

  describe("write gating", () => {
    it("denies writes when the rule disables write", () => {
      // Arrange
      provider.set(makeRule({ write: false }));

      // Act
      const evaluation = service.evaluateWrite(SOURCE, TABLE, "update");

      // Assert
      expect(evaluation.allowed).toBe(false);
      expect(evaluation.reason).toBe(`Write access denied for ${SOURCE}/${TABLE}`);
    });

    it("denies delete when deleteRecords is disabled even though write is enabled", () => {
      // Arrange
      provider.set(makeRule({ write: true, deleteRecords: false }));

      // Act
      const evaluation = service.evaluateWrite(SOURCE, TABLE, "delete");

      // Assert
      expect(evaluation.allowed).toBe(false);
      expect(evaluation.reason).toBe(`Delete access denied for ${SOURCE}/${TABLE}`);
    });

    it("allows delete when deleteRecords is enabled", () => {
      provider.set(makeRule({ write: true, deleteRecords: true }));
      expect(service.evaluateWrite(SOURCE, TABLE, "delete").allowed).toBe(true);
    });

    it("throws a typed PermissionDeniedError from assertCanWrite", () => {
      // Arrange
      provider.set(makeRule({ write: false }));

      // Act
      let caught: unknown;
      try {
        service.assertCanWrite(SOURCE, TABLE, "append");
      } catch (error: unknown) {
        caught = error;
      }

      // Assert
      expect(caught).toBeInstanceOf(PermissionDeniedError);
      expect((caught as PermissionDeniedError).name).toBe("PermissionDeniedError");
    });
  });

  describe("confirmation requirements", () => {
    it("flags confirmation per action, including bulk_update", () => {
      // Arrange
      provider.set(makeRule({ requireConfirmationFor: ["update", "bulk_update"] }));

      // Act + Assert
      expect(service.evaluateWrite(SOURCE, TABLE, "update").requiresConfirmation).toBe(true);
      expect(service.evaluateWrite(SOURCE, TABLE, "bulk_update").requiresConfirmation).toBe(true);
      expect(service.evaluateWrite(SOURCE, TABLE, "append").requiresConfirmation).toBe(false);
      expect(service.assertCanWrite(SOURCE, TABLE, "bulk_update")).toEqual({ requiresConfirmation: true });
    });
  });

  describe("fresh rule reads", () => {
    it("applies a rule change made between two evaluations", () => {
      // Arrange
      provider.set(makeRule({ write: true }));
      expect(service.assertCanWrite(SOURCE, TABLE, "update")).toEqual({ requiresConfirmation: false });

      // Act: the desktop app revokes write access.
      provider.set(makeRule({ write: false }));

      // Assert: no caching, the next call sees the new rule.
      expect(() => service.assertCanWrite(SOURCE, TABLE, "update")).toThrow(PermissionDeniedError);
    });
  });
});
