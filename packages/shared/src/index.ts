export type DataSourceKind = "google_sheets" | "provider" | "mock";

export type DataSource = {
  id: string;
  kind: DataSourceKind;
  name: string;
};

export type TableRef = {
  sourceId: string;
  tableId: string;
  name: string;
};

export type FieldSchema = {
  name: string;
  type: "string" | "number" | "boolean" | "date" | "email" | "enum" | "unknown";
  required?: boolean;
  readonly?: boolean;
  enumValues?: string[];
};

export type TableSchema = {
  sourceId: string;
  tableId: string;
  name: string;
  fields: FieldSchema[];
};

export type TableRecord = {
  id: string;
  fields: Record<string, unknown>;
};

export type RecordPatch = {
  recordId: string;
  fields: Record<string, unknown>;
};

export type ChangeType = "append" | "update" | "delete";
export type ConfirmationAction = "append" | "update" | "delete" | "bulk_update" | "formula_change";

export type PermissionRule = {
  sourceId: string;
  tableId?: string;
  read: boolean;
  write: boolean;
  deleteRecords: boolean;
  requireConfirmationFor: ConfirmationAction[];
};

export type PendingChange = {
  id: string;
  sourceId: string;
  tableId: string;
  type: ChangeType;
  createdAt: string;
  status: "pending" | "committed" | "rejected";
  diff: unknown;
};

export type AuditEvent = {
  id: string;
  timestamp: string;
  actor: "user" | "agent" | "system";
  action: string;
  sourceId?: string;
  tableId?: string;
  metadata?: Record<string, unknown>;
};

export type ReadTableOptions = {
  limit?: number;
  offset?: number;
};

export interface TableConnector {
  kind: DataSourceKind;
  listSources(): Promise<DataSource[]>;
  listTables(sourceId: string): Promise<TableRef[]>;
  describeTable(sourceId: string, tableId: string): Promise<TableSchema>;
  readTable(sourceId: string, tableId: string, options?: ReadTableOptions): Promise<TableRecord[]>;
  findRecords(sourceId: string, tableId: string, query: string): Promise<TableRecord[]>;
  appendRecords(sourceId: string, tableId: string, records: Array<Record<string, unknown>>): Promise<TableRecord[]>;
  updateRecords(sourceId: string, tableId: string, patches: RecordPatch[]): Promise<TableRecord[]>;
}
