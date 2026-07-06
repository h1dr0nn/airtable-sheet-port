import { AuditService, ChangeService, ConnectorRegistry, PermissionService, SchemaService } from "@sheet-port/core";
import { MockConnector } from "@sheet-port/mock-connector";
import {
  AuditStore,
  ChangeStore,
  HeartbeatStore,
  MockDataStore,
  PermissionStore,
  SourceStore,
  openSheetPortDb,
  resolveDbPath
} from "@sheet-port/storage";

export type AppContext = {
  dbPath: string;
  audit: AuditService;
  changes: ChangeService;
  permissions: PermissionService;
  registry: ConnectorRegistry;
  schemas: SchemaService;
  heartbeat: HeartbeatStore;
};

/**
 * Wires services to the shared SQLite database. Permission rules, pending
 * changes, audit events, and mock data all live in the DB (seeded by
 * packages/storage/seed.sql), so desktop decisions apply here immediately.
 */
export function createAppContext(): AppContext {
  const dbPath = resolveDbPath();
  const db = openSheetPortDb(dbPath);

  const sources = new SourceStore(db);
  const registry = new ConnectorRegistry((sourceId) => sources.getKind(sourceId));
  registry.register(new MockConnector(new MockDataStore(db), sources));

  const permissions = new PermissionService(new PermissionStore(db));

  return {
    dbPath,
    audit: new AuditService(new AuditStore(db)),
    changes: new ChangeService(new ChangeStore(db), permissions, registry),
    permissions,
    registry,
    schemas: new SchemaService(),
    heartbeat: new HeartbeatStore(db)
  };
}
