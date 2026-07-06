# MCP Tools

The Rust sidecar (`crates/sheet-port-mcp`) registers exactly 9 tools. All input
schemas are provider-neutral (generated via `schemars`, with every bound enforced in
`src/args.rs`); none expose raw Google or provider APIs. Every tool returns a single
text content block containing pretty-printed JSON with the shapes below. Every call
writes an audit event (actor `agent`).

Shared types (TypeScript notation; defined in `crates/sheet-port-core/src/types.rs`
and mirrored for the frontend in `packages/shared`):

```ts
type DataSource   = { id: string; kind: "google_sheets" | "provider" | "mock"; name: string; status?: "connected" | "placeholder" | "error" };
type TableRef     = { sourceId: string; tableId: string; name: string };
type TableSchema  = { sourceId: string; tableId: string; name: string; fields: FieldSchema[] };
type FieldSchema  = { name: string; type: "string" | "number" | "boolean" | "date" | "email" | "enum" | "unknown"; required?: boolean; readonly?: boolean; enumValues?: string[] };
type TableRecord  = { id: string; fields: Record<string, unknown> };
type PendingChange = {
  id: string;                     // "chg_" + UUID
  sourceId: string;
  tableId: string;
  type: "append" | "update" | "delete";
  createdAt: string;              // ISO timestamp
  status: "pending" | "approved" | "committed" | "rejected";
  requiresConfirmation: boolean;  // snapshot of the permission rule at preview time
  diff: unknown;                  // see per-tool shapes below
  decidedAt?: string;
  decidedBy?: "user" | "policy";
  committedAt?: string;
};
type AuditEvent   = { id: string; timestamp: string; actor: "user" | "agent" | "system"; action: string; sourceId?: string; tableId?: string; metadata?: Record<string, unknown> };
```

Examples below match the seed data in `crates/sheet-port-core/sql/seed.sql`: one
connected mock
source `mock-source` ("Demo Workspace") with table `customers`, and a permission rule
that allows read + write and requires confirmation for `append`, `update`, `delete`,
and `bulk_update`.

## `list_sources`

Purpose: list connected data sources (read-only).

Input schema: none (empty object).

Output shape: `{ "sources": DataSource[] }`

Permission required: none beyond local MCP access (audited).

Example call:

```json
{}
```

Example response:

```json
{
  "sources": [
    { "id": "mock-source", "kind": "mock", "name": "Demo Workspace", "status": "connected" }
  ]
}
```

Only sources whose `kind` has a registered connector are returned; the seeded Google
Sheets and provider placeholder rows are visible in the desktop UI but not here.

## `list_tables`

Purpose: list tables for a data source (read-only).

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |

Output shape: `{ "tables": TableRef[] }`

Permission required: `read` on the source.

Example call:

```json
{ "sourceId": "mock-source" }
```

Example response:

```json
{
  "tables": [
    { "sourceId": "mock-source", "tableId": "customers", "name": "Customers" }
  ]
}
```

## `describe_table`

Purpose: return the field schema of a table (read-only).

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 |

Output shape: `{ "schema": TableSchema }`

Permission required: `read` on the source/table.

Example call:

```json
{ "sourceId": "mock-source", "tableId": "customers" }
```

Example response:

```json
{
  "schema": {
    "sourceId": "mock-source",
    "tableId": "customers",
    "name": "Customers",
    "fields": [
      { "name": "Name", "type": "string", "required": true },
      { "name": "Email", "type": "email" },
      { "name": "Plan", "type": "enum", "enumValues": ["free", "pro", "enterprise"] },
      { "name": "Seats", "type": "number" },
      { "name": "Active", "type": "boolean" }
    ]
  }
}
```

## `read_table`

Purpose: read bounded records from a table (read-only).

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 |
| `limit` | integer | 1 to 500, default 100 |
| `offset` | integer | >= 0, default 0 |

Output shape: `{ "records": TableRecord[] }` (ordered by stored position)

Permission required: `read` on the source/table.

Example call:

```json
{ "sourceId": "mock-source", "tableId": "customers", "limit": 2, "offset": 0 }
```

Example response:

```json
{
  "records": [
    { "id": "rec_seed_1", "fields": { "Name": "Aurora Labs", "Email": "ops@auroralabs.dev", "Plan": "pro", "Seats": 24, "Active": true } },
    { "id": "rec_seed_2", "fields": { "Name": "Basalt Co", "Email": "it@basalt.co", "Plan": "free", "Seats": 3, "Active": true } }
  ]
}
```

## `find_records`

Purpose: case-insensitive text search across all field values (read-only).

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 |
| `query` | string | 1 to 200 characters |

Output shape: `{ "records": TableRecord[] }` (mock connector caps results at 100)

Permission required: `read` on the source/table.

Example call:

```json
{ "sourceId": "mock-source", "tableId": "customers", "query": "aurora" }
```

Example response:

```json
{
  "records": [
    { "id": "rec_seed_1", "fields": { "Name": "Aurora Labs", "Email": "ops@auroralabs.dev", "Plan": "pro", "Seats": 24, "Active": true } }
  ]
}
```

## `preview_update_records`

Purpose: create a pending update change and return its diff. Nothing is written to the
table until `commit_change`.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 |
| `patches` | array of `{ recordId: string (min 1), fields: object }` | 1 to 100 items |

Output shape: `{ "change": PendingChange, "requiresConfirmation": boolean }`

Diff shape (in `change.diff`): one entry per patch,
`[{ "recordId", "before": fields | null, "after": merged fields }]`. `before` is `null`
when the record id does not currently exist.

Permission required: `read` AND `write`. Read is checked first because the diff exposes
current record values. When `patches.length > 20` (`BULK_UPDATE_THRESHOLD`), the write
is evaluated as the `bulk_update` action instead of `update`, so a rule can require
confirmation (or deny) bulk edits specifically.

`requiresConfirmation` is `true` when the matching permission rule lists the evaluated
action in `requireConfirmationFor`. If it is `true`, the user must approve the change in
the desktop app before `commit_change` will succeed.

Example call:

```json
{
  "sourceId": "mock-source",
  "tableId": "customers",
  "patches": [
    { "recordId": "rec_seed_2", "fields": { "Plan": "pro", "Seats": 10 } }
  ]
}
```

Example response:

```json
{
  "change": {
    "id": "chg_1f0d3c62-9a44-4b1e-9a1f-b1d2c3e4f5a6",
    "sourceId": "mock-source",
    "tableId": "customers",
    "type": "update",
    "createdAt": "2026-07-06T09:15:00.000Z",
    "status": "pending",
    "requiresConfirmation": true,
    "diff": [
      {
        "recordId": "rec_seed_2",
        "before": { "Name": "Basalt Co", "Email": "it@basalt.co", "Plan": "free", "Seats": 3, "Active": true },
        "after": { "Name": "Basalt Co", "Email": "it@basalt.co", "Plan": "pro", "Seats": 10, "Active": true }
      }
    ]
  },
  "requiresConfirmation": true
}
```

## `append_records`

Purpose: create a pending append change and return its diff. Nothing is written to the
table until `commit_change`.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 |
| `records` | array of objects (field name -> value) | 1 to 100 items |

Output shape: `{ "change": PendingChange, "requiresConfirmation": boolean }`

Diff shape (in `change.diff`): `{ "after": records }`.

Permission required: `write` (evaluated as the `append` action).

Example call:

```json
{
  "sourceId": "mock-source",
  "tableId": "customers",
  "records": [
    { "Name": "Dune Harbor", "Email": "hello@duneharbor.io", "Plan": "free", "Seats": 2, "Active": true }
  ]
}
```

Example response:

```json
{
  "change": {
    "id": "chg_7a2b9c10-3d4e-4f56-8a9b-0c1d2e3f4a5b",
    "sourceId": "mock-source",
    "tableId": "customers",
    "type": "append",
    "createdAt": "2026-07-06T09:16:00.000Z",
    "status": "pending",
    "requiresConfirmation": true,
    "diff": {
      "after": [
        { "Name": "Dune Harbor", "Email": "hello@duneharbor.io", "Plan": "free", "Seats": 2, "Active": true }
      ]
    }
  },
  "requiresConfirmation": true
}
```

## `commit_change`

Purpose: apply a previously previewed change. This is the only tool that writes to a
table.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `changeId` | string | min length 1 |

Output shape: `{ "change": PendingChange, "records": TableRecord[] }` where
`change.status` is `"committed"` and `records` are the written rows (updates return only
the records that actually existed; appends return the new records with generated ids).

Permission required: `write`, re-checked at commit time against fresh rules with the
same action evaluated at preview (`update` vs `bulk_update` vs `append`). Revoking write
access after preview blocks the commit.

Enforcement (see `docs/ipc.md`): when the change was created with
`requiresConfirmation: true`, the user must approve it in the desktop app first. Changes
without a confirmation requirement auto-approve at commit with `decidedBy: "policy"`.

Example call:

```json
{ "changeId": "chg_1f0d3c62-9a44-4b1e-9a1f-b1d2c3e4f5a6" }
```

Example response (after the user approved in the desktop app):

```json
{
  "change": {
    "id": "chg_1f0d3c62-9a44-4b1e-9a1f-b1d2c3e4f5a6",
    "sourceId": "mock-source",
    "tableId": "customers",
    "type": "update",
    "createdAt": "2026-07-06T09:15:00.000Z",
    "status": "committed",
    "requiresConfirmation": true,
    "diff": [
      {
        "recordId": "rec_seed_2",
        "before": { "Name": "Basalt Co", "Email": "it@basalt.co", "Plan": "free", "Seats": 3, "Active": true },
        "after": { "Name": "Basalt Co", "Email": "it@basalt.co", "Plan": "pro", "Seats": 10, "Active": true }
      }
    ],
    "decidedAt": "2026-07-06T09:17:21.412Z",
    "decidedBy": "user",
    "committedAt": "2026-07-06T09:18:05.003Z"
  },
  "records": [
    { "id": "rec_seed_2", "fields": { "Name": "Basalt Co", "Email": "it@basalt.co", "Plan": "pro", "Seats": 10, "Active": true } }
  ]
}
```

Error cases (returned as tool errors with these messages):

| Condition | Error |
|---|---|
| Change id does not exist | `Unknown change <changeId>` |
| Needs approval and status is not `approved` | `Change <changeId> requires user approval in the Airtable - Sheet Port desktop app before commit` |
| User rejected it in the desktop app | `Change <changeId> was rejected in the desktop app and cannot be committed` |
| Already committed | `Change <changeId> is already committed` |
| Write permission revoked since preview | `Write access denied for <sourceId>/<tableId>` |

## `get_audit_log`

Purpose: return recent audit events, newest first (read-only).

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `limit` | integer | 1 to 500, default 100 |

Output shape: `{ "events": AuditEvent[] }`

Permission required: none beyond local MCP access (the call itself is audited).

Example call:

```json
{ "limit": 3 }
```

Example response:

```json
{
  "events": [
    {
      "id": "evt_0b9c8d7e-6f5a-4b3c-2d1e-0f9a8b7c6d5e",
      "timestamp": "2026-07-06T09:18:05.010Z",
      "actor": "agent",
      "action": "commit_change",
      "sourceId": "mock-source",
      "tableId": "customers",
      "metadata": { "changeId": "chg_1f0d3c62-9a44-4b1e-9a1f-b1d2c3e4f5a6", "recordCount": 1 }
    },
    {
      "id": "evt_1c2d3e4f-5a6b-7c8d-9e0f-1a2b3c4d5e6f",
      "timestamp": "2026-07-06T09:17:21.415Z",
      "actor": "user",
      "action": "change_approved",
      "sourceId": "mock-source",
      "tableId": "customers",
      "metadata": { "changeId": "chg_1f0d3c62-9a44-4b1e-9a1f-b1d2c3e4f5a6" }
    },
    {
      "id": "evt_2d3e4f5a-6b7c-8d9e-0f1a-2b3c4d5e6f7a",
      "timestamp": "2026-07-06T09:15:00.005Z",
      "actor": "agent",
      "action": "preview_update_records",
      "sourceId": "mock-source",
      "tableId": "customers",
      "metadata": { "changeId": "chg_1f0d3c62-9a44-4b1e-9a1f-b1d2c3e4f5a6", "patchCount": 1, "requiresConfirmation": true }
    }
  ]
}
```

## Current Limitations

- No delete tool exists; delete changes are typed in the schema but rejected by the
  change pipeline.
- Only the mock connector is functional; calls against non-mock sources fail with a
  routing error.
