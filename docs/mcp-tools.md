# MCP Tools

All tools use provider-neutral schemas. None expose raw Google or provider APIs.

## `list_sources`

Purpose: list connected data sources.

Input schema:

```json
{}
```

Output schema: `{ "sources": DataSource[] }`

Permission required: none beyond local MCP access.

Example call:

```json
{}
```

Example response:

```json
{ "sources": [{ "id": "mock-source", "kind": "mock", "name": "Demo Workspace" }] }
```

## `list_tables`

Purpose: list tables for a source.

Input schema:

```json
{ "sourceId": "mock-source" }
```

Output schema: `{ "tables": TableRef[] }`

Permission required: read.

## `describe_table`

Purpose: return fields and metadata for a table.

Input schema:

```json
{ "sourceId": "mock-source", "tableId": "customers" }
```

Output schema: `{ "schema": TableSchema }`

Permission required: read.

## `read_table`

Purpose: read bounded rows from a table.

Input schema:

```json
{ "sourceId": "mock-source", "tableId": "customers", "limit": 50, "offset": 0 }
```

Output schema: `{ "records": TableRecord[] }`

Permission required: read.

## `find_records`

Purpose: search records by text query.

Input schema:

```json
{ "sourceId": "mock-source", "tableId": "customers", "query": "active" }
```

Output schema: `{ "records": TableRecord[] }`

Permission required: read.

## `preview_update_records`

Purpose: create a pending update change and return a diff.

Input schema:

```json
{
  "sourceId": "mock-source",
  "tableId": "customers",
  "patches": [{ "recordId": "rec_1", "fields": { "Status": "Inactive" } }]
}
```

Output schema: `{ "change": PendingChange }`

Permission required: write.

## `append_records`

Purpose: create a pending append change and return a diff.

Input schema:

```json
{
  "sourceId": "mock-source",
  "tableId": "customers",
  "records": [{ "Name": "New Customer", "Email": "new@example.com" }]
}
```

Output schema: `{ "change": PendingChange }`

Permission required: write.

## `commit_change`

Purpose: commit a pending append or update.

Input schema:

```json
{ "changeId": "chg_123" }
```

Output schema: `{ "change": PendingChange, "records": TableRecord[] }`

Permission required: write. Delete changes also require `deleteRecords`, but deletes are not implemented in the MVP.

## `get_audit_log`

Purpose: return recent audit events.

Input schema:

```json
{ "limit": 50 }
```

Output schema: `{ "events": AuditEvent[] }`

Permission required: local MCP access.

## Current Limitations

- Audit and pending changes are in-memory.
- Desktop confirmation state is not yet attached to `commit_change`.
