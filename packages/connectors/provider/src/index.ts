import type { DataSource, ReadTableOptions, RecordPatch, TableConnector, TableRecord, TableRef, TableSchema } from "@sheet-port/shared";

export type ProviderConnectorConfig = {
  // TODO: Load API key or OAuth token from secure desktop-owned storage.
  sourceIds?: string[];
};

export class ProviderConnector implements TableConnector {
  readonly kind = "provider" as const;

  constructor(private readonly config: ProviderConnectorConfig = {}) {}

  async listSources(): Promise<DataSource[]> {
    return (this.config.sourceIds ?? []).map((id) => ({
      id: `provider:${id}`,
      kind: this.kind,
      name: `Provider Source ${id}`
    }));
  }

  async listTables(_sourceId: string): Promise<TableRef[]> {
    throw new Error("Provider connector TODO: discover tables after auth is implemented");
  }

  async describeTable(_sourceId: string, _tableId: string): Promise<TableSchema> {
    throw new Error("Provider connector TODO: map provider field metadata to TableSchema");
  }

  async readTable(_sourceId: string, _tableId: string, _options?: ReadTableOptions): Promise<TableRecord[]> {
    throw new Error("Provider connector TODO: read records with pagination and rate limiting");
  }

  async findRecords(_sourceId: string, _tableId: string, _query: string): Promise<TableRecord[]> {
    throw new Error("Provider connector TODO: search/filter records safely");
  }

  async appendRecords(_sourceId: string, _tableId: string, _records: Array<Record<string, unknown>>): Promise<TableRecord[]> {
    throw new Error("Provider connector TODO: create records after preview and policy approval");
  }

  async updateRecords(_sourceId: string, _tableId: string, _patches: RecordPatch[]): Promise<TableRecord[]> {
    throw new Error("Provider connector TODO: update records after preview and policy approval");
  }
}
