import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { BULK_UPDATE_THRESHOLD } from "@sheet-port/shared";
import { z } from "zod";
import type { AppContext } from "./context.js";

const sourceTableSchema = {
  sourceId: z.string().min(1),
  tableId: z.string().min(1)
};

const READ_ONLY = { readOnlyHint: true } as const;

export function createServer(context: AppContext): McpServer {
  const server = new McpServer({
    name: "sheet-port",
    version: "0.1.0"
  });

  server.registerTool(
    "list_sources",
    { description: "List connected data sources.", annotations: READ_ONLY },
    async () => {
      const sources = await context.registry.listSources();
      context.audit.record({ actor: "agent", action: "list_sources", metadata: { count: sources.length } });
      return json({ sources });
    }
  );

  server.registerTool(
    "list_tables",
    {
      description: "List tables for a data source.",
      inputSchema: { sourceId: z.string().min(1) },
      annotations: READ_ONLY
    },
    async ({ sourceId }) => {
      context.permissions.assertCanRead(sourceId);
      const tables = await context.registry.listTables(sourceId);
      context.audit.record({ actor: "agent", action: "list_tables", sourceId, metadata: { count: tables.length } });
      return json({ tables });
    }
  );

  server.registerTool(
    "describe_table",
    { description: "Describe a table schema.", inputSchema: sourceTableSchema, annotations: READ_ONLY },
    async ({ sourceId, tableId }) => {
      context.permissions.assertCanRead(sourceId, tableId);
      const schema = await context.registry.describeTable(sourceId, tableId);
      context.schemas.set(schema);
      context.audit.record({ actor: "agent", action: "describe_table", sourceId, tableId });
      return json({ schema });
    }
  );

  server.registerTool(
    "read_table",
    {
      description: "Read bounded records from a table.",
      inputSchema: {
        ...sourceTableSchema,
        limit: z.number().int().min(1).max(500).default(100),
        offset: z.number().int().min(0).default(0)
      },
      annotations: READ_ONLY
    },
    async ({ sourceId, tableId, limit, offset }) => {
      context.permissions.assertCanRead(sourceId, tableId);
      const records = await context.registry.readTable(sourceId, tableId, { limit, offset });
      context.audit.record({ actor: "agent", action: "read_table", sourceId, tableId, metadata: { limit, offset, count: records.length } });
      return json({ records });
    }
  );

  server.registerTool(
    "find_records",
    {
      description: "Find records by text query.",
      inputSchema: { ...sourceTableSchema, query: z.string().min(1).max(200) },
      annotations: READ_ONLY
    },
    async ({ sourceId, tableId, query }) => {
      context.permissions.assertCanRead(sourceId, tableId);
      const records = await context.registry.findRecords(sourceId, tableId, query);
      context.audit.record({ actor: "agent", action: "find_records", sourceId, tableId, metadata: { query, count: records.length } });
      return json({ records });
    }
  );

  server.registerTool(
    "preview_update_records",
    {
      description: "Create a pending update change and return its diff.",
      inputSchema: {
        ...sourceTableSchema,
        patches: z.array(z.object({ recordId: z.string().min(1), fields: z.record(z.unknown()) })).min(1).max(100)
      }
    },
    async ({ sourceId, tableId, patches }) => {
      // Read permission is checked first: the diff exposes current record values.
      context.permissions.assertCanRead(sourceId, tableId);
      const action = patches.length > BULK_UPDATE_THRESHOLD ? "bulk_update" : "update";
      const policy = context.permissions.assertCanWrite(sourceId, tableId, action);
      const change = await context.changes.createUpdateChange(sourceId, tableId, patches, policy.requiresConfirmation);
      context.audit.record({
        actor: "agent",
        action: "preview_update_records",
        sourceId,
        tableId,
        metadata: { changeId: change.id, patchCount: patches.length, requiresConfirmation: policy.requiresConfirmation }
      });
      return json({ change, requiresConfirmation: policy.requiresConfirmation });
    }
  );

  server.registerTool(
    "append_records",
    {
      description: "Create a pending append change and return its diff.",
      inputSchema: { ...sourceTableSchema, records: z.array(z.record(z.unknown())).min(1).max(100) }
    },
    async ({ sourceId, tableId, records }) => {
      const policy = context.permissions.assertCanWrite(sourceId, tableId, "append");
      const change = context.changes.createAppendChange(sourceId, tableId, records, policy.requiresConfirmation);
      context.audit.record({
        actor: "agent",
        action: "append_records_preview",
        sourceId,
        tableId,
        metadata: { changeId: change.id, recordCount: records.length, requiresConfirmation: policy.requiresConfirmation }
      });
      return json({ change, requiresConfirmation: policy.requiresConfirmation });
    }
  );

  server.registerTool(
    "commit_change",
    {
      description: "Commit a pending change after policy checks.",
      inputSchema: { changeId: z.string().min(1) }
    },
    async ({ changeId }) => {
      const pending = context.changes.get(changeId);
      if (!pending) {
        throw new Error(`Unknown change ${changeId}`);
      }
      context.permissions.assertCanWrite(pending.sourceId, pending.tableId, pending.type);
      // ChangeService.commit enforces the approval flow (docs/ipc.md) and
      // re-checks permissions with the exact preview action before writing.
      const result = await context.changes.commit(changeId);
      context.audit.record({
        actor: "agent",
        action: "commit_change",
        sourceId: result.change.sourceId,
        tableId: result.change.tableId,
        metadata: { changeId, recordCount: result.records.length }
      });
      return json(result);
    }
  );

  server.registerTool(
    "get_audit_log",
    {
      description: "Return recent audit events.",
      inputSchema: { limit: z.number().int().min(1).max(500).default(100) },
      annotations: READ_ONLY
    },
    async ({ limit }) => {
      context.audit.record({ actor: "agent", action: "get_audit_log", metadata: { limit } });
      return json({ events: context.audit.list(limit) });
    }
  );

  return server;
}

function json(data: unknown) {
  return {
    content: [
      {
        type: "text" as const,
        text: JSON.stringify(data, null, 2)
      }
    ]
  };
}
