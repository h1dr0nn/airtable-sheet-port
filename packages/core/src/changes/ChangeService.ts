import { BULK_UPDATE_THRESHOLD } from "@sheet-port/shared";
import type { ChangeStatus, ChangeType, PendingChange, RecordPatch, TableRecord, WriteAction } from "@sheet-port/shared";
import type { ConnectorRegistry } from "../connectors/ConnectorRegistry.js";
import type { PermissionService } from "../permissions/PermissionService.js";

/** Internal write payload; persisted alongside a change but never returned to agents. */
export type ChangePayload =
  | { type: "append"; records: Array<Record<string, unknown>> }
  | { type: "update"; patches: RecordPatch[] }
  | { type: "delete"; recordIds: string[] };

export type ChangeDecider = "user" | "policy";

/** Persistence abstraction implemented by @sheet-port/storage. */
export interface ChangeStorePort {
  insert(change: PendingChange, payload: ChangePayload): void;
  get(changeId: string): PendingChange | undefined;
  getPayload(changeId: string): ChangePayload | undefined;
  /** Newest first; all statuses when omitted. */
  list(status?: ChangeStatus): PendingChange[];
  /** Atomic guarded transition (UPDATE ... WHERE status = from); false when the guard missed. */
  transition(changeId: string, from: ChangeStatus, to: ChangeStatus, decidedBy: ChangeDecider): boolean;
  /** Atomic approved -> committed; false when the change was not in the approved state. */
  markCommitted(changeId: string): boolean;
}

export class ChangeService {
  constructor(
    private readonly store: ChangeStorePort,
    private readonly permissions: PermissionService,
    private readonly registry: ConnectorRegistry
  ) {}

  createAppendChange(
    sourceId: string,
    tableId: string,
    records: Array<Record<string, unknown>>,
    requiresConfirmation: boolean
  ): PendingChange {
    return this.createChange(sourceId, tableId, "append", { type: "append", records }, { after: records }, requiresConfirmation);
  }

  async createUpdateChange(
    sourceId: string,
    tableId: string,
    patches: RecordPatch[],
    requiresConfirmation: boolean
  ): Promise<PendingChange> {
    const currentRecords = await this.registry.readTable(sourceId, tableId);
    const beforeById = new Map(currentRecords.map((record) => [record.id, record]));
    const diff = patches.map((patch) => ({
      recordId: patch.recordId,
      before: beforeById.get(patch.recordId)?.fields ?? null,
      after: { ...(beforeById.get(patch.recordId)?.fields ?? {}), ...patch.fields }
    }));
    return this.createChange(sourceId, tableId, "update", { type: "update", patches }, diff, requiresConfirmation);
  }

  get(changeId: string): PendingChange | undefined {
    return this.store.get(changeId);
  }

  list(status?: ChangeStatus): PendingChange[] {
    return this.store.list(status);
  }

  /**
   * Enforcement per docs/ipc.md "Confirmation enforcement": the desktop app
   * approves/rejects rows in the shared DB; this method reads fresh state so
   * the decision applies across processes without direct IPC.
   */
  async commit(changeId: string): Promise<{ change: PendingChange; records: TableRecord[] }> {
    const change = this.store.get(changeId);
    if (!change) {
      throw new Error(`Unknown change ${changeId}`);
    }
    if (change.status === "rejected") {
      throw new Error(`Change ${changeId} was rejected in the desktop app and cannot be committed`);
    }
    if (change.status === "committed") {
      throw new Error(`Change ${changeId} is already committed`);
    }
    if (change.requiresConfirmation && change.status !== "approved") {
      throw new Error(`Change ${changeId} requires user approval in the Airtable - Sheet Port desktop app before commit`);
    }

    const payload = this.store.getPayload(changeId);
    if (!payload) {
      throw new Error(`Change ${changeId} has no stored payload`);
    }

    // Permission rules may have changed since preview; re-check at commit time.
    this.permissions.assertCanWrite(change.sourceId, change.tableId, commitAction(change.type, payload));

    if (change.status === "pending") {
      // Only reachable when requiresConfirmation is false: policy auto-approves.
      const transitioned = this.store.transition(changeId, "pending", "approved", "policy");
      if (!transitioned) {
        const current = this.store.get(changeId);
        throw new Error(`Change ${changeId} is ${current?.status ?? "missing"} and cannot be committed`);
      }
    }

    const records = await this.execute(change, payload);
    if (!this.store.markCommitted(changeId)) {
      throw new Error(`Change ${changeId} could not be marked committed (state changed concurrently)`);
    }
    const committed = this.store.get(changeId);
    if (!committed) {
      throw new Error(`Change ${changeId} disappeared after commit`);
    }
    return { change: committed, records };
  }

  private async execute(change: PendingChange, payload: ChangePayload): Promise<TableRecord[]> {
    if (payload.type === "append") {
      return this.registry.appendRecords(change.sourceId, change.tableId, payload.records);
    }
    if (payload.type === "update") {
      return this.registry.updateRecords(change.sourceId, change.tableId, payload.patches);
    }
    throw new Error("Delete changes are not implemented in the MVP");
  }

  private createChange(
    sourceId: string,
    tableId: string,
    type: ChangeType,
    payload: ChangePayload,
    diff: unknown,
    requiresConfirmation: boolean
  ): PendingChange {
    const change: PendingChange = {
      id: `chg_${crypto.randomUUID()}`,
      sourceId,
      tableId,
      type,
      createdAt: new Date().toISOString(),
      status: "pending",
      requiresConfirmation,
      diff
    };
    this.store.insert(change, payload);
    return change;
  }
}

/** Re-derive the evaluated action so commit re-checks the same policy as preview. */
function commitAction(type: ChangeType, payload: ChangePayload): WriteAction {
  if (payload.type === "update" && payload.patches.length > BULK_UPDATE_THRESHOLD) {
    return "bulk_update";
  }
  return type;
}
