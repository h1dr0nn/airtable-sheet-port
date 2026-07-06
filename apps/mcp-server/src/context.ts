import { AuditService, ChangeService, ConnectorRegistry, PermissionService, SchemaService } from "@sheet-port/core";
import { MockConnector } from "@sheet-port/mock-connector";
import type { PermissionRule } from "@sheet-port/shared";

export type AppContext = {
  audit: AuditService;
  changes: ChangeService;
  permissions: PermissionService;
  registry: ConnectorRegistry;
  schemas: SchemaService;
};

export function createAppContext(): AppContext {
  const registry = new ConnectorRegistry();
  registry.register(new MockConnector());

  const defaultRules: PermissionRule[] = [
    {
      sourceId: "mock-source",
      tableId: "customers",
      read: true,
      write: true,
      deleteRecords: false,
      requireConfirmationFor: ["append", "update", "delete", "bulk_update", "formula_change"]
    }
  ];

  return {
    audit: new AuditService(),
    changes: new ChangeService(),
    permissions: new PermissionService(defaultRules),
    registry,
    schemas: new SchemaService()
  };
}
