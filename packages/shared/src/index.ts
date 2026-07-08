export type DataSourceKind = "google_sheets" | "provider" | "mock";

export type SourceStatus = "connected" | "placeholder" | "error";

export type DataSource = {
  id: string;
  kind: DataSourceKind;
  name: string;
  status?: SourceStatus;
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

export type ChangeType = "append" | "update" | "delete" | "format";
export type ConfirmationAction =
  | "append"
  | "update"
  | "delete"
  | "bulk_update"
  | "formula_change"
  | "format";

/** Write action evaluated against permission rules; wider than ChangeType. */
export type WriteAction = "append" | "update" | "delete" | "bulk_update" | "format";

export type HorizontalAlignment = "LEFT" | "CENTER" | "RIGHT";

export type NumberFormatType =
  | "TEXT"
  | "NUMBER"
  | "PERCENT"
  | "CURRENCY"
  | "DATE"
  | "TIME"
  | "DATE_TIME"
  | "SCIENTIFIC";

export type BorderStyle = "none" | "all" | "outer" | "bottom";

/** One cell-format operation over an A1 range; only the set properties change. */
export type CellFormat = {
  range: string;
  bold?: boolean;
  italic?: boolean;
  fontSize?: number;
  fontColor?: string;
  backgroundColor?: string;
  horizontalAlignment?: HorizontalAlignment;
  numberFormat?: string;
  numberFormatType?: NumberFormatType;
  wrap?: boolean;
  border?: BorderStyle;
};

export type ColumnWidth = { column: string; pixels: number };

/** A staged formatting change; also the agent-visible diff of a format change. */
export type FormatPlan = {
  formats?: CellFormat[];
  freezeRows?: number;
  freezeColumns?: number;
  columnWidths?: ColumnWidth[];
};

/** Update previews touching more than this many records are treated as bulk_update. */
export const BULK_UPDATE_THRESHOLD = 20;

export type PermissionRule = {
  sourceId: string;
  tableId?: string;
  read: boolean;
  write: boolean;
  deleteRecords: boolean;
  requireConfirmationFor: ConfirmationAction[];
};

export type ChangeStatus = "pending" | "approved" | "committed" | "rejected";

export type PendingChange = {
  id: string;
  sourceId: string;
  tableId: string;
  type: ChangeType;
  createdAt: string;
  status: ChangeStatus;
  /** True when the matching permission rule requires user confirmation before commit. */
  requiresConfirmation: boolean;
  diff: unknown;
  decidedAt?: string;
  decidedBy?: "user" | "policy";
  committedAt?: string;
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
