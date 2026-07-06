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
  GoogleConfig,
  GoogleConnectResult,
  IpcApi,
  PermissionRuleRow,
  SavePermissionRule,
  TablePage,
  TokenStatus
} from "./ipc.js";

// In-memory stand-in for the Rust backend so `vite dev` in a plain browser is
// fully clickable. Mirrors the schema v2 empty state: a fresh database has no
// sources, rules, changes, or audit rows. Clicking Connect on the Google
// Sheets card links a fake account after a short delay so every screen stays
// explorable without OAuth.

const DEMO_LATENCY_MS = 150;
const GOOGLE_CONNECT_DELAY_MS = 1_500;
const DEFAULT_READ_LIMIT = 100;
const MAX_READ_LIMIT = 500;
const DEFAULT_AUDIT_LIMIT = 100;
const MAX_AUDIT_LIMIT = 500;
const CHANGES_LIST_LIMIT = 200;
const DEMO_MCP_PID = 48213;
const HEARTBEAT_AGE_MS = 4_000;

// Mirrors sheet-port-core: google::GOOGLE_SOURCE_ID and the source name format.
const GOOGLE_SOURCE_ID = "google-sheets";
const DEMO_GOOGLE_EMAIL = "demo.user@gmail.com";
// Pre-filled so the Connect button is immediately clickable in the preview.
const DEMO_GOOGLE_CLIENT_ID = "000000000000-demo.apps.googleusercontent.com";

const wait = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
const delay = () => wait(DEMO_LATENCY_MS);
const nowIso = () => new Date().toISOString();

const SHEET_TABLE: TableRef = { sourceId: GOOGLE_SOURCE_ID, tableId: "leads", name: "Leads" };

const SHEET_SCHEMA: TableSchema = {
  sourceId: GOOGLE_SOURCE_ID,
  tableId: SHEET_TABLE.tableId,
  name: SHEET_TABLE.name,
  fields: [
    { name: "Name", type: "string", required: true },
    { name: "Email", type: "email" },
    { name: "Stage", type: "enum", enumValues: ["new", "qualified", "won"] },
    { name: "Value", type: "number" }
  ]
};

const SHEET_RECORDS: readonly TableRecord[] = [
  { id: "row_2", fields: { Name: "Aurora Labs", Email: "ops@auroralabs.dev", Stage: "qualified", Value: 12_000 } },
  { id: "row_3", fields: { Name: "Basalt Co", Email: "it@basalt.co", Stage: "new", Value: 3_500 } },
  { id: "row_4", fields: { Name: "Cirrus Retail", Email: "admin@cirrus.shop", Stage: "won", Value: 48_000 } },
  { id: "row_5", fields: { Name: "Drift Systems", Email: "hello@drift.example", Stage: "new", Value: 7_250 } }
];

// One pending preview appears alongside the connected source so the Changes
// and Dashboard screens stay clickable; it references only demo-real rows.
function buildSeededChange(): PendingChange {
  const before = SHEET_RECORDS[1];
  return {
    id: "chg_demo_google_update",
    sourceId: GOOGLE_SOURCE_ID,
    tableId: SHEET_TABLE.tableId,
    type: "update",
    createdAt: nowIso(),
    status: "pending",
    requiresConfirmation: true,
    diff: [
      {
        recordId: before?.id ?? "row_3",
        before: { ...before?.fields },
        after: { ...before?.fields, Stage: "qualified", Value: 9_000 }
      }
    ]
  };
}

type DemoOptions = {
  /** Override the pre-seeded client id; pass null to model a fresh install. */
  googleClientId?: string | null;
};

/** Builds an isolated demo backend; exported for tests. */
export function createDemoIpc(options: DemoOptions = {}): IpcApi {
  let googleClientId =
    options.googleClientId === undefined ? DEMO_GOOGLE_CLIENT_ID : options.googleClientId;
  let googleEmail: string | null = null;

  let sources: DataSource[] = [];
  let permissionRules: PermissionRuleRow[] = [];
  let nextRuleId = 1;
  let changes: PendingChange[] = [];
  let auditEvents: AuditEvent[] = [];
  let auditCounter = 0;

  const makeAuditId = () => {
    auditCounter += 1;
    return `evt_demo_${auditCounter}`;
  };

  const pushAudit = (event: Omit<AuditEvent, "id" | "timestamp">): void => {
    auditEvents = [...auditEvents, { ...event, id: makeAuditId(), timestamp: nowIso() }];
  };

  const isGoogleConnected = () => googleEmail !== null;

  const decideChange = (changeId: string, status: "approved" | "rejected"): PendingChange => {
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
  };

  const newestChangeFirst = (a: PendingChange, b: PendingChange) =>
    b.createdAt.localeCompare(a.createdAt);
  const newestEventFirst = (a: AuditEvent, b: AuditEvent) => b.timestamp.localeCompare(a.timestamp);

  return {
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
      return isGoogleConnected() && sourceId === GOOGLE_SOURCE_ID ? [{ ...SHEET_TABLE }] : [];
    },
    async describeTable(sourceId: string, tableId: string): Promise<TableSchema> {
      await delay();
      if (!isGoogleConnected() || sourceId !== SHEET_SCHEMA.sourceId || tableId !== SHEET_SCHEMA.tableId) {
        throw new Error(`Unknown table ${sourceId}/${tableId}`);
      }
      return SHEET_SCHEMA;
    },
    async readTable(sourceId, tableId, limit, offset): Promise<TablePage> {
      await delay();
      const isSheet =
        isGoogleConnected() && sourceId === SHEET_TABLE.sourceId && tableId === SHEET_TABLE.tableId;
      const records = isSheet ? [...SHEET_RECORDS] : [];
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
      return { googleSheets: isGoogleConnected(), provider: false };
    },
    async getGoogleConfig(): Promise<GoogleConfig> {
      await delay();
      return { clientId: googleClientId, connectedEmail: googleEmail };
    },
    async setGoogleClientId(clientId: string): Promise<void> {
      await delay();
      const trimmed = clientId.trim();
      if (trimmed === "") {
        throw new Error("Google client ID must not be empty");
      }
      googleClientId = trimmed;
    },
    async googleConnect(): Promise<GoogleConnectResult> {
      if (googleClientId === null) {
        throw new Error("Google client ID is not configured. Set it in the desktop app settings");
      }
      // Stands in for the real browser consent round-trip.
      await wait(GOOGLE_CONNECT_DELAY_MS);
      const wasConnected = isGoogleConnected();
      googleEmail = DEMO_GOOGLE_EMAIL;
      sources = [
        ...sources.filter((source) => source.id !== GOOGLE_SOURCE_ID),
        {
          id: GOOGLE_SOURCE_ID,
          kind: "google_sheets",
          name: `Google Sheets (${DEMO_GOOGLE_EMAIL})`,
          status: "connected"
        }
      ];
      pushAudit({
        actor: "user",
        action: "google_connected",
        sourceId: GOOGLE_SOURCE_ID,
        metadata: { email: DEMO_GOOGLE_EMAIL }
      });
      if (!wasConnected && !changes.some((change) => change.sourceId === GOOGLE_SOURCE_ID)) {
        const seeded = buildSeededChange();
        changes = [...changes, seeded];
        pushAudit({
          actor: "agent",
          action: "preview_update_records",
          sourceId: seeded.sourceId,
          tableId: seeded.tableId,
          metadata: { changeId: seeded.id, records: 1 }
        });
      }
      return { email: DEMO_GOOGLE_EMAIL };
    },
    async googleDisconnect(): Promise<void> {
      await delay();
      // Idempotent, like core::google::disconnect.
      googleEmail = null;
      sources = sources.filter((source) => source.id !== GOOGLE_SOURCE_ID);
      pushAudit({ actor: "user", action: "google_disconnected", sourceId: GOOGLE_SOURCE_ID });
    }
  };
}

// Plain-browser dev preview instance shared by lib/ipc.ts.
export const demoIpc: IpcApi = createDemoIpc();
