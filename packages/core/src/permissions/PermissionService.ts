import type { ChangeType, PermissionRule } from "@sheet-port/shared";

export class PermissionDeniedError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "PermissionDeniedError";
  }
}

export class PermissionService {
  private rules: PermissionRule[];

  constructor(rules: PermissionRule[] = []) {
    this.rules = rules;
  }

  setRules(rules: PermissionRule[]): void {
    this.rules = rules;
  }

  listRules(): PermissionRule[] {
    return [...this.rules];
  }

  assertCanRead(sourceId: string, tableId?: string): void {
    const rule = this.findRule(sourceId, tableId);
    if (!rule?.read) {
      throw new PermissionDeniedError(`Read access denied for ${sourceId}${tableId ? `/${tableId}` : ""}`);
    }
  }

  assertCanWrite(sourceId: string, tableId: string, type: ChangeType): { requiresConfirmation: boolean } {
    const rule = this.findRule(sourceId, tableId);
    if (!rule?.write) {
      throw new PermissionDeniedError(`Write access denied for ${sourceId}/${tableId}`);
    }
    if (type === "delete" && !rule.deleteRecords) {
      throw new PermissionDeniedError(`Delete access denied for ${sourceId}/${tableId}`);
    }
    return { requiresConfirmation: rule.requireConfirmationFor.includes(type) };
  }

  private findRule(sourceId: string, tableId?: string): PermissionRule | undefined {
    return (
      this.rules.find((rule) => rule.sourceId === sourceId && rule.tableId === tableId) ??
      this.rules.find((rule) => rule.sourceId === sourceId && !rule.tableId)
    );
  }
}
