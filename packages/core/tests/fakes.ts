import type {
  ChangeStatus,
  DataSource,
  DataSourceKind,
  PendingChange,
  PermissionRule,
  ReadTableOptions,
  RecordPatch,
  TableConnector,
  TableRecord,
  TableRef,
  TableSchema
} from "@sheet-port/shared";
import type { ChangeDecider, ChangePayload, ChangeStorePort, PermissionRuleProvider } from "../src/index.js";

/** In-memory PermissionRuleProvider with the documented table-over-source precedence. */
export class InMemoryRuleProvider implements PermissionRuleProvider {
  private rules: PermissionRule[] = [];

  set(rule: PermissionRule): void {
    this.rules = [
      ...this.rules.filter((existing) => !(existing.sourceId === rule.sourceId && existing.tableId === rule.tableId)),
      rule
    ];
  }

  clear(): void {
    this.rules = [];
  }

  findRule(sourceId: string, tableId?: string): PermissionRule | undefined {
    const exact =
      tableId !== undefined
        ? this.rules.find((rule) => rule.sourceId === sourceId && rule.tableId === tableId)
        : undefined;
    return exact ?? this.rules.find((rule) => rule.sourceId === sourceId && rule.tableId === undefined);
  }
}

/** In-memory ChangeStorePort honoring the guarded-transition contract. */
export class InMemoryChangeStore implements ChangeStorePort {
  private readonly changes = new Map<string, PendingChange>();
  private readonly payloads = new Map<string, ChangePayload>();

  insert(change: PendingChange, payload: ChangePayload): void {
    this.changes.set(change.id, { ...change });
    this.payloads.set(change.id, payload);
  }

  get(changeId: string): PendingChange | undefined {
    const change = this.changes.get(changeId);
    return change ? { ...change } : undefined;
  }

  getPayload(changeId: string): ChangePayload | undefined {
    return this.payloads.get(changeId);
  }

  list(status?: ChangeStatus): PendingChange[] {
    const all = [...this.changes.values()].sort((a, b) => b.createdAt.localeCompare(a.createdAt));
    return (status === undefined ? all : all.filter((change) => change.status === status)).map((change) => ({
      ...change
    }));
  }

  transition(changeId: string, from: ChangeStatus, to: ChangeStatus, decidedBy: ChangeDecider): boolean {
    const change = this.changes.get(changeId);
    if (!change || change.status !== from) {
      return false;
    }
    this.changes.set(changeId, { ...change, status: to, decidedAt: new Date().toISOString(), decidedBy });
    return true;
  }

  markCommitted(changeId: string): boolean {
    const change = this.changes.get(changeId);
    if (!change || change.status !== "approved") {
      return false;
    }
    this.changes.set(changeId, { ...change, status: "committed", committedAt: new Date().toISOString() });
    return true;
  }

  /** Test-only backdoor simulating a desktop-app decision written to the shared DB. */
  forceStatus(changeId: string, status: ChangeStatus): void {
    const change = this.changes.get(changeId);
    if (!change) {
      throw new Error(`forceStatus: unknown change ${changeId}`);
    }
    this.changes.set(changeId, { ...change, status });
  }
}

/** In-memory TableConnector; record data keyed by sourceId/tableId. */
export class FakeConnector implements TableConnector {
  readonly kind: DataSourceKind;
  private readonly tables = new Map<string, TableRecord[]>();
  private nextId = 1;

  constructor(
    kind: DataSourceKind,
    private readonly sources: DataSource[]
  ) {
    this.kind = kind;
  }

  seed(sourceId: string, tableId: string, records: TableRecord[]): void {
    this.tables.set(this.key(sourceId, tableId), records.map(cloneRecord));
  }

  async listSources(): Promise<DataSource[]> {
    return this.sources.map((source) => ({ ...source }));
  }

  async listTables(sourceId: string): Promise<TableRef[]> {
    return [...this.tables.keys()]
      .filter((key) => key.startsWith(`${sourceId}/`))
      .map((key) => {
        const tableId = key.slice(sourceId.length + 1);
        return { sourceId, tableId, name: tableId };
      });
  }

  async describeTable(sourceId: string, tableId: string): Promise<TableSchema> {
    return { sourceId, tableId, name: tableId, fields: [] };
  }

  async readTable(sourceId: string, tableId: string, options: ReadTableOptions = {}): Promise<TableRecord[]> {
    const records = this.records(sourceId, tableId).map(cloneRecord);
    const offset = options.offset ?? 0;
    return options.limit === undefined ? records.slice(offset) : records.slice(offset, offset + options.limit);
  }

  async findRecords(sourceId: string, tableId: string, query: string): Promise<TableRecord[]> {
    const normalized = query.toLowerCase();
    return this.records(sourceId, tableId)
      .filter((record) => Object.values(record.fields).some((value) => String(value).toLowerCase().includes(normalized)))
      .map(cloneRecord);
  }

  async appendRecords(sourceId: string, tableId: string, records: Array<Record<string, unknown>>): Promise<TableRecord[]> {
    const appended = records.map((fields) => ({ id: `rec_fake_${this.nextId++}`, fields: { ...fields } }));
    this.tables.set(this.key(sourceId, tableId), [...this.records(sourceId, tableId), ...appended]);
    return appended.map(cloneRecord);
  }

  async updateRecords(sourceId: string, tableId: string, patches: RecordPatch[]): Promise<TableRecord[]> {
    const byId = new Map(patches.map((patch) => [patch.recordId, patch.fields]));
    const updated: TableRecord[] = [];
    const next = this.records(sourceId, tableId).map((record) => {
      const patchFields = byId.get(record.id);
      if (!patchFields) {
        return record;
      }
      const merged = { id: record.id, fields: { ...record.fields, ...patchFields } };
      updated.push(cloneRecord(merged));
      return merged;
    });
    this.tables.set(this.key(sourceId, tableId), next);
    return updated;
  }

  private records(sourceId: string, tableId: string): TableRecord[] {
    return this.tables.get(this.key(sourceId, tableId)) ?? [];
  }

  private key(sourceId: string, tableId: string): string {
    return `${sourceId}/${tableId}`;
  }
}

function cloneRecord(record: TableRecord): TableRecord {
  return { id: record.id, fields: { ...record.fields } };
}
