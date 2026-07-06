import type { DataSource, DataSourceKind, ReadTableOptions, RecordPatch, TableConnector, TableRecord, TableRef, TableSchema } from "@sheet-port/shared";

/** Maps a source id to its connector kind; backed by the sources table in the sidecar. */
export type ResolveSourceKind = (sourceId: string) => DataSourceKind | undefined;

export class ConnectorRegistry {
  private readonly connectors = new Map<DataSourceKind, TableConnector>();

  constructor(private readonly resolveKind: ResolveSourceKind) {}

  register(connector: TableConnector): void {
    this.connectors.set(connector.kind, connector);
  }

  async listSources(): Promise<DataSource[]> {
    const nested = await Promise.all([...this.connectors.values()].map((connector) => connector.listSources()));
    return nested.flat();
  }

  async listTables(sourceId: string): Promise<TableRef[]> {
    return this.forSource(sourceId).listTables(sourceId);
  }

  async describeTable(sourceId: string, tableId: string): Promise<TableSchema> {
    return this.forSource(sourceId).describeTable(sourceId, tableId);
  }

  async readTable(sourceId: string, tableId: string, options?: ReadTableOptions): Promise<TableRecord[]> {
    return this.forSource(sourceId).readTable(sourceId, tableId, options);
  }

  async findRecords(sourceId: string, tableId: string, query: string): Promise<TableRecord[]> {
    return this.forSource(sourceId).findRecords(sourceId, tableId, query);
  }

  async appendRecords(sourceId: string, tableId: string, records: Array<Record<string, unknown>>): Promise<TableRecord[]> {
    return this.forSource(sourceId).appendRecords(sourceId, tableId, records);
  }

  async updateRecords(sourceId: string, tableId: string, patches: RecordPatch[]): Promise<TableRecord[]> {
    return this.forSource(sourceId).updateRecords(sourceId, tableId, patches);
  }

  private forSource(sourceId: string): TableConnector {
    const kind = this.resolveKind(sourceId);
    if (!kind) {
      throw new Error(`Unknown source ${sourceId}`);
    }
    const connector = this.connectors.get(kind);
    if (!connector) {
      throw new Error(`No connector registered for source kind ${kind} (source ${sourceId})`);
    }
    return connector;
  }
}
