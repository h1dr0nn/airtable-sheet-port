import type { DataSource, ReadTableOptions, RecordPatch, TableConnector, TableRecord, TableRef, TableSchema } from "@sheet-port/shared";
import type { MockDataStore, SourceStore } from "@sheet-port/storage";

/** Cap for text search results so agents never receive unbounded payloads. */
const FIND_RECORDS_LIMIT = 100;

/**
 * Mock connector backed by the shared SQLite database, so the desktop UI and
 * the MCP sidecar observe the same tables, records, and committed changes.
 */
export class MockConnector implements TableConnector {
  readonly kind = "mock" as const;

  constructor(
    private readonly data: MockDataStore,
    private readonly sources: SourceStore
  ) {}

  async listSources(): Promise<DataSource[]> {
    return this.sources.list().filter((source) => source.kind === this.kind);
  }

  async listTables(sourceId: string): Promise<TableRef[]> {
    this.assertSource(sourceId);
    return this.data.listTables(sourceId);
  }

  async describeTable(sourceId: string, tableId: string): Promise<TableSchema> {
    return this.requireTable(sourceId, tableId);
  }

  async readTable(sourceId: string, tableId: string, options: ReadTableOptions = {}): Promise<TableRecord[]> {
    this.requireTable(sourceId, tableId);
    return this.data.listRecords(sourceId, tableId, options).records;
  }

  async findRecords(sourceId: string, tableId: string, query: string): Promise<TableRecord[]> {
    this.requireTable(sourceId, tableId);
    const normalized = query.toLowerCase();
    const { records } = this.data.listRecords(sourceId, tableId);
    return records
      .filter((record) => Object.values(record.fields).some((value) => String(value).toLowerCase().includes(normalized)))
      .slice(0, FIND_RECORDS_LIMIT);
  }

  async appendRecords(sourceId: string, tableId: string, records: Array<Record<string, unknown>>): Promise<TableRecord[]> {
    this.requireTable(sourceId, tableId);
    return this.data.appendRecords(sourceId, tableId, records);
  }

  async updateRecords(sourceId: string, tableId: string, patches: RecordPatch[]): Promise<TableRecord[]> {
    this.requireTable(sourceId, tableId);
    return this.data.updateRecords(sourceId, tableId, patches);
  }

  private assertSource(sourceId: string): void {
    if (this.sources.getKind(sourceId) !== this.kind) {
      throw new Error(`Unknown mock source ${sourceId}`);
    }
  }

  private requireTable(sourceId: string, tableId: string): TableSchema {
    this.assertSource(sourceId);
    const schema = this.data.getTable(sourceId, tableId);
    if (!schema) {
      throw new Error(`Unknown mock table ${sourceId}/${tableId}`);
    }
    return schema;
  }
}
