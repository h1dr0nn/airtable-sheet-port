import type {
  AuditEvent,
  DataSource,
  PendingChange,
  TableRecord,
  TableRef,
  TableSchema
} from "@sheet-port/shared";
import type {
  AppStatus,
  IpcApi,
  PermissionRuleRow,
  SavePermissionRule,
  TablePage,
  TokenStatus
} from "./ipc.js";

// In-memory stand-in for the Rust backend so `vite dev` in a plain browser is
// fully clickable. Fixtures mirror packages/storage/seed.sql.

const DEMO_LATENCY_MS = 150;
const DEFAULT_READ_LIMIT = 100;
const MAX_READ_LIMIT = 500;
const DEFAULT_AUDIT_LIMIT = 100;
const MAX_AUDIT_LIMIT = 500;
const CHANGES_LIST_LIMIT = 200;
const DEMO_MCP_PID = 48213;
const HEARTBEAT_AGE_MS = 4_000;

const delay = () => new Promise<void>((resolve) => setTimeout(resolve, DEMO_LATENCY_MS));
const nowIso = () => new Date().toISOString();
const minutesAgo = (minutes: number) => new Date(Date.now() - minutes * 60_000).toISOString();

const sources: DataSource[] = [
  { id: "mock-source", kind: "mock", name: "Demo Workspace", status: "connected" },
  { id: "google-placeholder", kind: "google_sheets", name: "Google Sheets (connect soon)", status: "placeholder" },
  { id: "provider-placeholder", kind: "provider", name: "Additional provider (connect soon)", status: "placeholder" }
];

const customersSchema: TableSchema = {
  sourceId: "mock-source",
  tableId: "customers",
  name: "Customers",
  fields: [
    { name: "Name", type: "string", required: true },
    { name: "Email", type: "email" },
    { name: "Plan", type: "enum", enumValues: ["free", "pro", "enterprise"] },
    { name: "Seats", type: "number" },
    { name: "Active", type: "boolean" }
  ]
};

const DEMO_COMPANIES = [
  "Drift Systems", "Ember Analytics", "Fjord Logistics", "Granite Works", "Helix Bio",
  "Iris Media", "Juniper Cloud", "Krill Robotics", "Lumen Grid", "Mesa Labs",
  "Nimbus Retail", "Onyx Finance", "Pico Devices", "Quartz Legal", "Rove Travel"
] as const;
const PLAN_CYCLE = ["free", "pro", "enterprise"] as const;

function buildDemoRecords(): TableRecord[] {
  const seeded: TableRecord[] = [
    { id: "rec_seed_1", fields: { Name: "Aurora Labs", Email: "ops@auroralabs.dev", Plan: "pro", Seats: 24, Active: true } },
    { id: "rec_seed_2", fields: { Name: "Basalt Co", Email: "it@basalt.co", Plan: "free", Seats: 3, Active: true } },
    { id: "rec_seed_3", fields: { Name: "Cirrus Retail", Email: "admin@cirrus.shop", Plan: "enterprise", Seats: 180, Active: false } }
  ];
  const extras = Array.from({ length: 30 }, (_, index) => {
    const company = DEMO_COMPANIES[index % DEMO_COMPANIES.length] ?? "Vertex Demo";
    const name = `${company} ${Math.floor(index / DEMO_COMPANIES.length) + 1}`;
    const slug = name.toLowerCase().replace(/[^a-z0-9]+/g, "");
    return {
      id: `rec_demo_${index + 4}`,
      fields: {
        Name: name,
        Email: `hello@${slug}.example`,
        Plan: PLAN_CYCLE[index % PLAN_CYCLE.length] ?? "free",
        Seats: (index % 12) * 5 + 2,
        Active: index % 4 !== 0
      }
    } satisfies TableRecord;
  });
  return [...seeded, ...extras];
}

const tableRecords = new Map<string, TableRecord[]>([["mock-source/customers", buildDemoRecords()]]);

let permissionRules: PermissionRuleRow[] = [
  {
    id: 1,
    sourceId: "mock-source",
    tableId: "customers",
    read: true,
    write: true,
    deleteRecords: false,
    requireConfirmationFor: ["append", "update", "delete", "bulk_update"],
    updatedAt: minutesAgo(90)
  }
];
let nextRuleId = 2;

let changes: PendingChange[] = [
  {
    id: "chg_demo_update",
    sourceId: "mock-source",
    tableId: "customers",
    type: "update",
    createdAt: minutesAgo(6),
    status: "pending",
    requiresConfirmation: true,
    diff: [
      {
        recordId: "rec_seed_2",
        before: { Name: "Basalt Co", Email: "it@basalt.co", Plan: "free", Seats: 3, Active: true },
        after: { Name: "Basalt Co", Email: "it@basalt.co", Plan: "pro", Seats: 12, Active: true }
      }
    ]
  },
  {
    id: "chg_demo_append",
    sourceId: "mock-source",
    tableId: "customers",
    type: "append",
    createdAt: minutesAgo(14),
    status: "pending",
    requiresConfirmation: true,
    diff: {
      after: [
        { Name: "Dune Analytics", Email: "team@dune.example", Plan: "free", Seats: 4, Active: true },
        { Name: "Echo Freight", Email: "ops@echofreight.example", Plan: "pro", Seats: 16, Active: true }
      ]
    }
  },
  {
    id: "chg_demo_committed",
    sourceId: "mock-source",
    tableId: "customers",
    type: "update",
    createdAt: minutesAgo(160),
    status: "committed",
    requiresConfirmation: false,
    diff: [
      {
        recordId: "rec_seed_3",
        before: { Name: "Cirrus Retail", Email: "admin@cirrus.shop", Plan: "enterprise", Seats: 160, Active: false },
        after: { Name: "Cirrus Retail", Email: "admin@cirrus.shop", Plan: "enterprise", Seats: 180, Active: false }
      }
    ],
    decidedAt: minutesAgo(158),
    decidedBy: "policy",
    committedAt: minutesAgo(158)
  },
  {
    id: "chg_demo_rejected",
    sourceId: "mock-source",
    tableId: "customers",
    type: "append",
    createdAt: minutesAgo(300),
    status: "rejected",
    requiresConfirmation: true,
    diff: { after: [{ Name: "Sparrow Ads", Email: "billing@sparrow.example", Plan: "enterprise", Seats: 400, Active: true }] },
    decidedAt: minutesAgo(295),
    decidedBy: "user"
  }
];

let auditCounter = 0;
const makeAuditId = () => {
  auditCounter += 1;
  return `evt_demo_${auditCounter}`;
};

let auditEvents: AuditEvent[] = [
  { id: makeAuditId(), timestamp: minutesAgo(300), actor: "agent", action: "preview_append_records", sourceId: "mock-source", tableId: "customers", metadata: { changeId: "chg_demo_rejected", records: 1 } },
  { id: makeAuditId(), timestamp: minutesAgo(295), actor: "user", action: "change_rejected", sourceId: "mock-source", tableId: "customers", metadata: { changeId: "chg_demo_rejected" } },
  { id: makeAuditId(), timestamp: minutesAgo(160), actor: "agent", action: "preview_update_records", sourceId: "mock-source", tableId: "customers", metadata: { changeId: "chg_demo_committed", records: 1 } },
  { id: makeAuditId(), timestamp: minutesAgo(158), actor: "agent", action: "change_committed", sourceId: "mock-source", tableId: "customers", metadata: { changeId: "chg_demo_committed", decidedBy: "policy" } },
  { id: makeAuditId(), timestamp: minutesAgo(90), actor: "user", action: "permission_rule_saved", sourceId: "mock-source", tableId: "customers", metadata: { read: true, write: true, deleteRecords: false } },
  { id: makeAuditId(), timestamp: minutesAgo(20), actor: "agent", action: "read_table", sourceId: "mock-source", tableId: "customers", metadata: { limit: 100, returned: 33 } },
  { id: makeAuditId(), timestamp: minutesAgo(14), actor: "agent", action: "preview_append_records", sourceId: "mock-source", tableId: "customers", metadata: { changeId: "chg_demo_append", records: 2 } },
  { id: makeAuditId(), timestamp: minutesAgo(6), actor: "agent", action: "preview_update_records", sourceId: "mock-source", tableId: "customers", metadata: { changeId: "chg_demo_update", records: 1 } }
];

function pushAudit(event: Omit<AuditEvent, "id" | "timestamp">): void {
  auditEvents = [...auditEvents, { ...event, id: makeAuditId(), timestamp: nowIso() }];
}

function decideChange(changeId: string, status: "approved" | "rejected"): PendingChange {
  const existing = changes.find((change) => change.id === changeId);
  if (!existing) {
    throw new Error(`Unknown change ${changeId}`);
  }
  if (existing.status !== "pending") {
    throw new Error(`Change ${changeId} is already ${existing.status}`);
  }
  const decided: PendingChange = { ...existing, status, decidedAt: nowIso(), decidedBy: "user" };
  changes = changes.map((change) => (change.id === changeId ? decided : change));
  pushAudit({
    actor: "user",
    action: status === "approved" ? "change_approved" : "change_rejected",
    sourceId: decided.sourceId,
    tableId: decided.tableId,
    metadata: { changeId }
  });
  return decided;
}

const newestChangeFirst = (a: PendingChange, b: PendingChange) => b.createdAt.localeCompare(a.createdAt);
const newestEventFirst = (a: AuditEvent, b: AuditEvent) => b.timestamp.localeCompare(a.timestamp);

export const demoIpc: IpcApi = {
  async getAppStatus(): Promise<AppStatus> {
    return {
      appVersion: "0.1.0 (browser demo)",
      dbPath: "C:\\Users\\demo\\AppData\\Roaming\\sheet-port\\sheet-port.db",
      mcpRunning: true,
      mcpPid: DEMO_MCP_PID,
      mcpLastSeen: new Date(Date.now() - HEARTBEAT_AGE_MS).toISOString(),
      pendingCount: changes.filter((change) => change.status === "pending").length
    };
  },
  async listSources(): Promise<DataSource[]> {
    await delay();
    return [...sources];
  },
  async listTables(sourceId: string): Promise<TableRef[]> {
    await delay();
    return sourceId === "mock-source"
      ? [{ sourceId: "mock-source", tableId: "customers", name: "Customers" }]
      : [];
  },
  async describeTable(sourceId: string, tableId: string): Promise<TableSchema> {
    await delay();
    if (sourceId !== customersSchema.sourceId || tableId !== customersSchema.tableId) {
      throw new Error(`Unknown table ${sourceId}/${tableId}`);
    }
    return customersSchema;
  },
  async readTable(sourceId, tableId, limit, offset): Promise<TablePage> {
    await delay();
    const records = tableRecords.get(`${sourceId}/${tableId}`) ?? [];
    const effectiveLimit = Math.min(limit ?? DEFAULT_READ_LIMIT, MAX_READ_LIMIT);
    const effectiveOffset = offset ?? 0;
    return {
      records: records.slice(effectiveOffset, effectiveOffset + effectiveLimit),
      total: records.length
    };
  },
  async listPermissionRules(): Promise<PermissionRuleRow[]> {
    await delay();
    return [...permissionRules];
  },
  async savePermissionRule(rule: SavePermissionRule): Promise<PermissionRuleRow> {
    await delay();
    if (rule.sourceId.trim() === "") {
      throw new Error("sourceId is required");
    }
    const matchesPair = (row: PermissionRuleRow) =>
      row.sourceId === rule.sourceId && row.tableId === rule.tableId;
    const existing = rule.id !== null
      ? permissionRules.find((row) => row.id === rule.id)
      : permissionRules.find(matchesPair);
    if (rule.id !== null && !existing) {
      throw new Error(`Unknown permission rule ${rule.id}`);
    }
    const saved: PermissionRuleRow = {
      id: existing ? existing.id : nextRuleId++,
      sourceId: rule.sourceId,
      tableId: rule.tableId,
      read: rule.read,
      write: rule.write,
      deleteRecords: rule.deleteRecords,
      requireConfirmationFor: [...rule.requireConfirmationFor],
      updatedAt: nowIso()
    };
    permissionRules = existing
      ? permissionRules.map((row) => (row.id === saved.id ? saved : row))
      : [...permissionRules, saved];
    pushAudit({
      actor: "user",
      action: "permission_rule_saved",
      sourceId: saved.sourceId,
      tableId: saved.tableId ?? undefined,
      metadata: { ...saved }
    });
    return saved;
  },
  async deletePermissionRule(id: number): Promise<void> {
    await delay();
    const existing = permissionRules.find((row) => row.id === id);
    if (!existing) {
      throw new Error(`Unknown permission rule ${id}`);
    }
    permissionRules = permissionRules.filter((row) => row.id !== id);
    pushAudit({
      actor: "user",
      action: "permission_rule_deleted",
      sourceId: existing.sourceId,
      tableId: existing.tableId ?? undefined,
      metadata: { id }
    });
  },
  async listChanges(status: string | null): Promise<PendingChange[]> {
    await delay();
    const filtered = status === null ? changes : changes.filter((change) => change.status === status);
    return [...filtered].sort(newestChangeFirst).slice(0, CHANGES_LIST_LIMIT);
  },
  async approveChange(changeId: string): Promise<PendingChange> {
    await delay();
    return decideChange(changeId, "approved");
  },
  async rejectChange(changeId: string): Promise<PendingChange> {
    await delay();
    return decideChange(changeId, "rejected");
  },
  async listAuditEvents(limit, offset): Promise<AuditEvent[]> {
    await delay();
    const effectiveLimit = Math.min(limit ?? DEFAULT_AUDIT_LIMIT, MAX_AUDIT_LIMIT);
    const effectiveOffset = offset ?? 0;
    return [...auditEvents]
      .sort(newestEventFirst)
      .slice(effectiveOffset, effectiveOffset + effectiveLimit);
  },
  async tokenStatus(): Promise<TokenStatus> {
    await delay();
    return { googleSheets: false, provider: false };
  }
};
