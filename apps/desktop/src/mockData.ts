import type { AuditEvent, DataSource, PendingChange, PermissionRule, TableRecord, TableSchema } from "@sheet-port/shared";

export const sources: DataSource[] = [
  { id: "mock-source", kind: "mock", name: "Demo Workspace" },
  { id: "google-placeholder", kind: "google_sheets", name: "Google Sheets (connect soon)" },
  { id: "provider-placeholder", kind: "provider", name: "Additional provider (connect soon)" }
];

export const schema: TableSchema = {
  sourceId: "mock-source",
  tableId: "customers",
  name: "Customers",
  fields: [
    { name: "Name", type: "string", required: true },
    { name: "Email", type: "email" },
    { name: "Status", type: "enum", enumValues: ["Active", "Paused", "Inactive"] },
    { name: "Seats", type: "number" },
    { name: "RenewalDate", type: "date" }
  ]
};

export const records: TableRecord[] = [
  { id: "rec_1", fields: { Name: "Acme Operations", Email: "ops@acme.example", Status: "Active", Seats: 18, RenewalDate: "2026-10-01" } },
  { id: "rec_2", fields: { Name: "Northwind Analytics", Email: "data@northwind.example", Status: "Paused", Seats: 7, RenewalDate: "2026-08-15" } },
  { id: "rec_3", fields: { Name: "Contoso Finance", Email: "finance@contoso.example", Status: "Active", Seats: 32, RenewalDate: "2027-01-20" } }
];

export const rules: PermissionRule[] = [
  {
    sourceId: "mock-source",
    tableId: "customers",
    read: true,
    write: true,
    deleteRecords: false,
    requireConfirmationFor: ["append", "update", "delete", "bulk_update", "formula_change"]
  }
];

export const pendingChanges: PendingChange[] = [
  {
    id: "chg_demo",
    sourceId: "mock-source",
    tableId: "customers",
    type: "update",
    createdAt: new Date().toISOString(),
    status: "pending",
    diff: [
      {
        recordId: "rec_2",
        before: { Status: "Paused" },
        after: { Status: "Active" }
      }
    ]
  }
];

export const auditEvents: AuditEvent[] = [
  {
    id: "evt_1",
    timestamp: new Date().toISOString(),
    actor: "agent",
    action: "preview_update_records",
    sourceId: "mock-source",
    tableId: "customers",
    metadata: { changeId: "chg_demo" }
  },
  {
    id: "evt_2",
    timestamp: new Date(Date.now() - 180000).toISOString(),
    actor: "agent",
    action: "read_table",
    sourceId: "mock-source",
    tableId: "customers",
    metadata: { count: 3 }
  }
];
