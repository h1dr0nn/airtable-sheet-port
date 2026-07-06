import type { DataSource, ReadTableOptions, RecordPatch, TableConnector, TableRecord, TableRef, TableSchema } from "@sheet-port/shared";

export type AirtableConnectorConfig = {
  // TODO: Load API key or OAuth token from secure desktop-owned storage.
  baseIds?: string[];
};

export class AirtableConnector implements TableConnector {
  readonly kind = "airtable" as const;

  constructor(private readonly config: AirtableConnectorConfig = {}) {}

  async listSources(): Promise<DataSource[]> {
    return (this.config.baseIds ?? []).map((id) => ({
      id: `airtable:${id}`,
      kind: this.kind,
      name: `Airtable Base ${id}`
    }));
  }

  async listTables(_sourceId: string): Promise<TableRef[]> {
    throw new Error("Airtable connector TODO: discover base tables after auth is implemented");
  }

  async describeTable(_sourceId: string, _tableId: string): Promise<TableSchema> {
    throw new Error("Airtable connector TODO: map Airtable field metadata to TableSchema");
  }

  async readTable(_sourceId: string, _tableId: string, _options?: ReadTableOptions): Promise<TableRecord[]> {
    throw new Error("Airtable connector TODO: read records with pagination and rate limiting");
  }

  async findRecords(_sourceId: string, _tableId: string, _query: string): Promise<TableRecord[]> {
    throw new Error("Airtable connector TODO: search/filter records safely");
  }

  async appendRecords(_sourceId: string, _tableId: string, _records: Array<Record<string, unknown>>): Promise<TableRecord[]> {
    throw new Error("Airtable connector TODO: create records after preview and policy approval");
  }

  async updateRecords(_sourceId: string, _tableId: string, _patches: RecordPatch[]): Promise<TableRecord[]> {
    throw new Error("Airtable connector TODO: update records after preview and policy approval");
  }
}
