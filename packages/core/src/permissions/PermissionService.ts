import type { PermissionRule, WriteAction } from "@sheet-port/shared";

export class PermissionDeniedError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PermissionDeniedError";
  }
}

/**
 * Rule lookup abstraction implemented by @sheet-port/storage. Implementations
 * must resolve with table-specific precedence: an exact (sourceId, tableId)
 * rule wins over a source-wide rule (no tableId).
 */
export interface PermissionRuleProvider {
  findRule(sourceId: string, tableId?: string): PermissionRule | undefined;
}

export type WriteEvaluation = {
  allowed: boolean;
  requiresConfirmation: boolean;
  reason?: string;
};

export class PermissionService {
  constructor(private readonly ruleProvider: PermissionRuleProvider) {}

  /**
   * Rules are read fresh from the provider on every evaluation (no caching)
   * so edits made in the desktop app apply to the sidecar immediately.
   */
  evaluateWrite(sourceId: string, tableId: string, action: WriteAction): WriteEvaluation {
    const rule = this.ruleProvider.findRule(sourceId, tableId);
    if (!rule?.write) {
      return { allowed: false, requiresConfirmation: false, reason: `Write access denied for ${sourceId}/${tableId}` };
    }
    if (action === "delete" && !rule.deleteRecords) {
      return { allowed: false, requiresConfirmation: false, reason: `Delete access denied for ${sourceId}/${tableId}` };
    }
    return { allowed: true, requiresConfirmation: rule.requireConfirmationFor.includes(action) };
  }

  assertCanRead(sourceId: string, tableId?: string): void {
    const rule = this.ruleProvider.findRule(sourceId, tableId);
    if (!rule?.read) {
      throw new PermissionDeniedError(`Read access denied for ${sourceId}${tableId ? `/${tableId}` : ""}`);
    }
  }

  assertCanWrite(sourceId: string, tableId: string, action: WriteAction): { requiresConfirmation: boolean } {
    const evaluation = this.evaluateWrite(sourceId, tableId, action);
    if (!evaluation.allowed) {
      throw new PermissionDeniedError(evaluation.reason ?? `Write access denied for ${sourceId}/${tableId}`);
    }
    return { requiresConfirmation: evaluation.requiresConfirmation };
  }
}
