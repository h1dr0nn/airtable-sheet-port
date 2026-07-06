import type { DataSource, ReadTableOptions, RecordPatch, TableConnector, TableRecord, TableRef, TableSchema } from "@sheet-port/shared";

export type GoogleSheetsConnectorConfig = {
  // TODO: Inject OAuth client and secure token lookup owned by the desktop app.
  spreadsheetIds?: string[];
};

export class GoogleSheetsConnector implements TableConnector {
  readonly kind = "google_sheets" as const;

  constructor(private readonly config: GoogleSheetsConnectorConfig = {}) {}

  async listSources(): Promise<DataSource[]> {
    return (this.config.spreadsheetIds ?? []).map((id) => ({
      id: `google_sheets:${id}`,
      kind: this.kind,
      name: `Google Sheet ${id}`
    }));
  }

  async listTables(_sourceId: string): Promise<TableRef[]> {
    throw new Error("Google Sheets connector TODO: discover sheet tabs/ranges after OAuth is implemented");
  }

  async describeTable(_sourceId: string, _tableId: string): Promise<TableSchema> {
    throw new Error("Google Sheets connector TODO: infer headers and schema from configured range");
  }

  async readTable(_sourceId: string, _tableId: string, _options?: ReadTableOptions): Promise<TableRecord[]> {
    throw new Error("Google Sheets connector TODO: read bounded values through googleapis");
  }

  async findRecords(_sourceId: string, _tableId: string, _query: string): Promise<TableRecord[]> {
    throw new Error("Google Sheets connector TODO: search records in fetched rows");
  }

  async appendRecords(_sourceId: string, _tableId: string, _records: Array<Record<string, unknown>>): Promise<TableRecord[]> {
    throw new Error("Google Sheets connector TODO: append rows after preview and policy approval");
  }

  async updateRecords(_sourceId: string, _tableId: string, _patches: RecordPatch[]): Promise<TableRecord[]> {
    throw new Error("Google Sheets connector TODO: update mapped row ranges after preview and policy approval");
  }
}
