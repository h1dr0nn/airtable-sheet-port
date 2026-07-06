import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { z } from "zod";
import type { AppContext } from "./context.js";

const sourceTableSchema = {
  sourceId: z.string().min(1),
  tableId: z.string().min(1)
};

export function createServer(context: AppContext): McpServer {
  const server = new McpServer({
    name: "sheet-port",
    version: "0.1.0"
  });

  server.tool("list_sources", "List connected data sources.", {}, async () => {
    const sources = await context.registry.listSources();
    context.audit.record({ actor: "agent", action: "list_sources", metadata: { count: sources.length } });
    return json({ sources });
  });

  server.tool("list_tables", "List tables for a data source.", { sourceId: z.string().min(1) }, async ({ sourceId }) => {
    context.permissions.assertCanRead(sourceId);
    const tables = await context.registry.listTables(sourceId);
    context.audit.record({ actor: "agent", action: "list_tables", sourceId, metadata: { count: tables.length } });
    return json({ tables });
  });

  server.tool("describe_table", "Describe a table schema.", sourceTableSchema, async ({ sourceId, tableId }) => {
    context.permissions.assertCanRead(sourceId, tableId);
    const schema = await context.registry.describeTable(sourceId, tableId);
    context.schemas.set(schema);
    context.audit.record({ actor: "agent", action: "describe_table", sourceId, tableId });
    return json({ schema });
  });

  server.tool(
    "read_table",
    "Read bounded records from a table.",
    { ...sourceTableSchema, limit: z.number().int().min(1).max(500).default(100), offset: z.number().int().min(0).default(0) },
    async ({ sourceId, tableId, limit, offset }) => {
      context.permissions.assertCanRead(sourceId, tableId);
      const records = await context.registry.readTable(sourceId, tableId, { limit, offset });
      context.audit.record({ actor: "agent", action: "read_table", sourceId, tableId, metadata: { limit, offset, count: records.length } });
      return json({ records });
    }
  );

  server.tool(
    "find_records",
    "Find records by text query.",
    { ...sourceTableSchema, query: z.string().min(1).max(200) },
    async ({ sourceId, tableId, query }) => {
      context.permissions.assertCanRead(sourceId, tableId);
      const records = await context.registry.findRecords(sourceId, tableId, query);
      context.audit.record({ actor: "agent", action: "find_records", sourceId, tableId, metadata: { query, count: records.length } });
      return json({ records });
    }
  );

  server.tool(
    "preview_update_records",
    "Create a pending update change and return its diff.",
    {
      ...sourceTableSchema,
      patches: z.array(z.object({ recordId: z.string().min(1), fields: z.record(z.unknown()) })).min(1).max(100)
    },
    async ({ sourceId, tableId, patches }) => {
      const policy = context.permissions.assertCanWrite(sourceId, tableId, patches.length > 20 ? "update" : "update");
      const change = await context.changes.createUpdateChange(sourceId, tableId, patches, context.registry);
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

  server.tool(
    "append_records",
    "Create a pending append change and return its diff.",
    { ...sourceTableSchema, records: z.array(z.record(z.unknown())).min(1).max(100) },
    async ({ sourceId, tableId, records }) => {
      const policy = context.permissions.assertCanWrite(sourceId, tableId, "append");
      const change = context.changes.createAppendChange(sourceId, tableId, records);
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

  server.tool("commit_change", "Commit a pending change after policy checks.", { changeId: z.string().min(1) }, async ({ changeId }) => {
    const pending = context.changes.get(changeId);
    if (!pending) {
      throw new Error(`Unknown change ${changeId}`);
    }
    context.permissions.assertCanWrite(pending.sourceId, pending.tableId, pending.type);
    const result = await context.changes.commit(changeId, context.registry);
    context.audit.record({
      actor: "agent",
      action: "commit_change",
      sourceId: result.change.sourceId,
      tableId: result.change.tableId,
      metadata: { changeId, recordCount: result.records.length }
    });
    return json(result);
  });

  server.tool("get_audit_log", "Return recent audit events.", { limit: z.number().int().min(1).max(500).default(100) }, async ({ limit }) => {
    return json({ events: context.audit.list(limit) });
  });

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
