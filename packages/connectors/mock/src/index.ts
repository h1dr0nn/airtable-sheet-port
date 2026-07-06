import type { DataSource, ReadTableOptions, RecordPatch, TableConnector, TableRecord, TableRef, TableSchema } from "@sheet-port/shared";

const source: DataSource = {
  id: "mock-source",
  kind: "mock",
  name: "Demo Workspace"
};

const table: TableRef = {
  sourceId: source.id,
  tableId: "customers",
  name: "Customers"
};

const schema: TableSchema = {
  sourceId: source.id,
  tableId: table.tableId,
  name: table.name,
  fields: [
    { name: "Name", type: "string", required: true },
    { name: "Email", type: "email" },
    { name: "Status", type: "enum", enumValues: ["Active", "Paused", "Inactive"] },
    { name: "Seats", type: "number" },
    { name: "RenewalDate", type: "date" }
  ]
};

export class MockConnector implements TableConnector {
  readonly kind = "mock" as const;

  private records: TableRecord[] = [
    {
      id: "rec_1",
      fields: {
        Name: "Acme Operations",
        Email: "ops@acme.example",
        Status: "Active",
        Seats: 18,
        RenewalDate: "2026-10-01"
      }
    },
    {
      id: "rec_2",
      fields: {
        Name: "Northwind Analytics",
        Email: "data@northwind.example",
        Status: "Paused",
        Seats: 7,
        RenewalDate: "2026-08-15"
      }
    },
    {
      id: "rec_3",
      fields: {
        Name: "Contoso Finance",
        Email: "finance@contoso.example",
        Status: "Active",
        Seats: 32,
        RenewalDate: "2027-01-20"
      }
    }
  ];

  async listSources(): Promise<DataSource[]> {
    return [source];
  }

  async listTables(sourceId: string): Promise<TableRef[]> {
    this.assertSource(sourceId);
    return [table];
  }

  async describeTable(sourceId: string, tableId: string): Promise<TableSchema> {
    this.assertTable(sourceId, tableId);
    return schema;
  }

  async readTable(sourceId: string, tableId: string, options: ReadTableOptions = {}): Promise<TableRecord[]> {
    this.assertTable(sourceId, tableId);
    const offset = options.offset ?? 0;
    const limit = options.limit ?? this.records.length;
    return this.records.slice(offset, offset + limit).map(cloneRecord);
  }

  async findRecords(sourceId: string, tableId: string, query: string): Promise<TableRecord[]> {
    this.assertTable(sourceId, tableId);
    const normalized = query.toLowerCase();
    return this.records
      .filter((record) => Object.values(record.fields).some((value) => String(value).toLowerCase().includes(normalized)))
      .map(cloneRecord);
  }

  async appendRecords(sourceId: string, tableId: string, records: Array<Record<string, unknown>>): Promise<TableRecord[]> {
    this.assertTable(sourceId, tableId);
    const appended = records.map((fields) => ({
      id: `rec_${crypto.randomUUID()}`,
      fields: { ...fields }
    }));
    this.records = [...this.records, ...appended];
    return appended.map(cloneRecord);
  }

  async updateRecords(sourceId: string, tableId: string, patches: RecordPatch[]): Promise<TableRecord[]> {
    this.assertTable(sourceId, tableId);
    const updated: TableRecord[] = [];
    const patchById = new Map(patches.map((patch) => [patch.recordId, patch]));
    this.records = this.records.map((record) => {
      const patch = patchById.get(record.id);
      if (!patch) {
        return record;
      }
      const next = { ...record, fields: { ...record.fields, ...patch.fields } };
      updated.push(next);
      return next;
    });
    return updated.map(cloneRecord);
  }

  private assertSource(sourceId: string): void {
    if (sourceId !== source.id) {
      throw new Error(`Unknown mock source ${sourceId}`);
    }
  }

  private assertTable(sourceId: string, tableId: string): void {
    this.assertSource(sourceId);
    if (tableId !== table.tableId) {
      throw new Error(`Unknown mock table ${tableId}`);
    }
  }
}

function cloneRecord(record: TableRecord): TableRecord {
  return { id: record.id, fields: { ...record.fields } };
}
