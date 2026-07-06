import type { ChangeType, PendingChange, RecordPatch, TableRecord } from "@sheet-port/shared";
import type { ConnectorRegistry } from "../connectors/ConnectorRegistry.js";

type ChangePayload =
  | { type: "append"; records: Array<Record<string, unknown>> }
  | { type: "update"; patches: RecordPatch[] }
  | { type: "delete"; recordIds: string[] };

type StoredChange = PendingChange & { payload: ChangePayload };

export class ChangeService {
  private readonly changes = new Map<string, StoredChange>();

  createAppendChange(sourceId: string, tableId: string, records: Array<Record<string, unknown>>): PendingChange {
    return this.createChange(sourceId, tableId, "append", { type: "append", records }, { after: records });
  }

  async createUpdateChange(sourceId: string, tableId: string, patches: RecordPatch[], registry: ConnectorRegistry): Promise<PendingChange> {
    const currentRecords = await registry.readTable(sourceId, tableId);
    const beforeById = new Map(currentRecords.map((record) => [record.id, record]));
    const diff = patches.map((patch) => ({
      recordId: patch.recordId,
      before: beforeById.get(patch.recordId)?.fields ?? null,
      after: { ...(beforeById.get(patch.recordId)?.fields ?? {}), ...patch.fields }
    }));
    return this.createChange(sourceId, tableId, "update", { type: "update", patches }, diff);
  }

  get(changeId: string): PendingChange | undefined {
    return this.changes.get(changeId);
  }

  list(): PendingChange[] {
    return [...this.changes.values()].map(({ payload: _payload, ...change }) => change);
  }

  async commit(changeId: string, registry: ConnectorRegistry): Promise<{ change: PendingChange; records: TableRecord[] }> {
    const stored = this.changes.get(changeId);
    if (!stored) {
      throw new Error(`Unknown change ${changeId}`);
    }
    if (stored.status !== "pending") {
      throw new Error(`Change ${changeId} is ${stored.status}`);
    }

    let records: TableRecord[];
    if (stored.payload.type === "append") {
      records = await registry.appendRecords(stored.sourceId, stored.tableId, stored.payload.records);
    } else if (stored.payload.type === "update") {
      records = await registry.updateRecords(stored.sourceId, stored.tableId, stored.payload.patches);
    } else {
      throw new Error("Delete changes are not implemented in the MVP");
    }

    const committed: StoredChange = { ...stored, status: "committed" };
    this.changes.set(changeId, committed);
    const { payload: _payload, ...change } = committed;
    return { change, records };
  }

  private createChange(sourceId: string, tableId: string, type: ChangeType, payload: ChangePayload, diff: unknown): PendingChange {
    const id = `chg_${crypto.randomUUID()}`;
    const change: StoredChange = {
      id,
      sourceId,
      tableId,
      type,
      createdAt: new Date().toISOString(),
      status: "pending",
      diff,
      payload
    };
    this.changes.set(id, change);
    const { payload: _payload, ...publicChange } = change;
    return publicChange;
  }
}
