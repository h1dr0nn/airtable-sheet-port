# MCP Tools

The Rust sidecar (`crates/sheet-port-mcp`) registers 17 tools. All input
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
  type: "append" | "update" | "delete" | "format";
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

## Google Sheets `tableId` forms

For a Google Sheets source, every table tool (`describe_table`, `read_table`,
`read_formulas`, `find_records`, `get_table_style`, `preview_update_records`,
`append_records`, `preview_format_table`) accepts the `tableId` in any
of these forms. This lets an agent paste a spreadsheet link the user shared and read the
exact tab without extra lookups:

| Form | Example | Tab selected |
|---|---|---|
| Full Google Sheets URL | `https://docs.google.com/spreadsheets/d/1BxiMVs.../edit?gid=1234567#gid=1234567` | the tab whose gid matches |
| URL without a gid | `https://docs.google.com/spreadsheets/d/1BxiMVs.../edit` | the first tab |
| Bare spreadsheet id | `1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms` | the first tab |
| `spreadsheetId:gid` | `1BxiMVs...:1234567` | the tab whose gid matches |
| `spreadsheetId:SheetName` | `1BxiMVs...:Q3 Summary` | the tab with that exact title |

Resolution fetches the spreadsheet metadata once
(`GET {SHEETS_ENDPOINT}/{id}?fields=properties.title,sheets.properties(sheetId,title)`)
to map a gid to its tab title or validate a tab name; every subsequent read/write range is
qualified by the resolved title (e.g. `'Q3 Summary'!A1:ZZ`). A gid or name that does not
exist returns a `NotFound` tool error. Only the spreadsheet id and tab selector are ever
extracted from a URL - the HTTP host and endpoint are fixed to the Google Sheets API for
the connected account's token (see `docs/security.md`, "Google Sheets URL parsing"). Ids
that do not look like a Google document id (too short, or containing URL/host characters)
are rejected before any request is made.

`list_tables` returns one entry per spreadsheet (the tableId is the spreadsheet id,
resolving to the first tab). To target a non-first tab, pass one of the tab-qualified
forms above rather than expecting a separate entry per tab; this keeps `list_tables`
bounded to a single Drive listing instead of one metadata call per spreadsheet.

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

Purpose: list the spreadsheets (tables) in a data source (read-only). For Google Sheets
each spreadsheet is one entry and its `tableId` is the spreadsheet id; use a tab-qualified
`tableId` (see "Google Sheets `tableId` forms") to read a specific tab.

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

Purpose: return the field schema of a table (read-only). For Google Sheets, `tableId` may
be a URL, spreadsheet id, or `spreadsheetId:gid` / `spreadsheetId:SheetName`; `name`
reflects the resolved tab (or the spreadsheet title when no tab is selected).

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

Purpose: read bounded records from a table (read-only). For Google Sheets, `tableId` may
be a URL, spreadsheet id, or `spreadsheetId:gid` / `spreadsheetId:SheetName` (see "Google
Sheets `tableId` forms").

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

## `read_formulas`

Purpose: read records like `read_table`, but with each cell's raw formula preserved - a
formula cell returns its `=...` text instead of the computed value (read-only). Use it
before overwriting cells that may be computed, so the formula logic is visible and not
clobbered. Same `tableId` forms and paging (`limit`/`offset`) as `read_table`.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 |
| `limit` | integer | optional, 1 to 500 (default 100) |
| `offset` | integer | optional, >= 0 (default 0) |

Output shape: `{ "records": TableRecord[] }` where a field value is the cell's formula
string when it holds one, else its literal value.

Permission required: `read` on the source/table. Only the Google Sheets connector supports
this; others return an Unsupported error.

## `read_cells`

Purpose: raw coordinate-level read (read-only). Returns every cell of the tab from row 1,
keyed by A1 column letter, with the real 1-based sheet row number on each row and NO
header/record interpretation. The escape hatch for document-style sheets (merged banner
rows, headers not on row 1, totals rows, multiple blocks) where `read_table` sees fewer
columns than the sheet actually has. Same `tableId` forms as `read_table`.

Input schema: same as `read_table` (`sourceId`, `tableId`, optional `limit`/`offset`
paging over sheet rows).

Output shape:

```json
{
  "columns": ["A", "B", "C"],
  "rows": [
    { "row": 1, "cells": { "A": "TASK CHECKLIST", "B": "", "C": "" } },
    { "row": 2, "cells": { "A": "Module", "B": "Task", "C": "Hours" } }
  ],
  "totalRows": 60
}
```

Permission required: `read` on the source/table.

## `preview_update_cells`

Purpose: stage coordinate-level writes to individual cells by A1 reference (e.g. set
`E48` to `350h`), returning the pending change; `commit_change` applies it. Values are
written with USER_ENTERED semantics - numbers parse as numbers, a leading `=` becomes a
live formula, anything else is text - matching what typing into the sheet would do. Use
whenever the record tools cannot address a cell.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 |
| `cells` | array of `{ cell, value }` | 1 to 100 items; `cell` is an A1 ref within A..ZZ, row >= 1; `value` at most 50000 chars |

Output shape: `{ "change": PendingChange, "requiresConfirmation": boolean }` (change type
`update_cells`; the diff lists each `{ cell, value }`).

Permission required: `write` (evaluated as the `update` action; more than 20 cells
escalates to `bulk_update` like a large record update).

## `find_records`

Purpose: case-insensitive text search across all field values (read-only). For Google
Sheets, `tableId` may be a URL, spreadsheet id, or `spreadsheetId:gid` /
`spreadsheetId:SheetName` (see "Google Sheets `tableId` forms").

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

## `get_table_style`

Purpose: read a tab's existing cell formatting so an agent can match it (read-only). It
returns the effective style of the header row and the first data row, plus the frozen
row/column counts and per-column pixel widths. Only properties actually set on a cell are
included, so the output stays compact. For Google Sheets, `tableId` may be a URL,
spreadsheet id, or `spreadsheetId:gid` / `spreadsheetId:SheetName`.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 |

Output shape: `{ "style": TableStyle }` where

```ts
type CellStyle = {
  column: string;                                    // A1 column letter
  bold?: boolean; italic?: boolean; fontSize?: number;
  fontColor?: string;                                // "#rrggbb"
  backgroundColor?: string;                          // "#rrggbb"
  horizontalAlignment?: "LEFT" | "CENTER" | "RIGHT";
  numberFormat?: string;                             // pattern
  wrap?: boolean;
};
type ColumnWidth = { column: string; pixels: number };
type TableStyle = {
  spreadsheetId: string;
  sheetTitle?: string;                               // omitted for the first tab
  frozenRowCount: number;
  frozenColumnCount: number;
  columnCount: number;                               // used (header) width
  header: CellStyle[];                               // row 1
  sample: CellStyle[];                               // row 2 (first data row)
  columnWidths: ColumnWidth[];
};
```

Permission required: `read` on the source/table. Only the Google Sheets connector
implements this; the mock connector returns a "does not support reading cell formatting"
error.

## `preview_update_records`

Purpose: create a pending update change and return its diff. Nothing is written to the
table until `commit_change`. For Google Sheets, `tableId` may be a URL, spreadsheet id, or
`spreadsheetId:gid` / `spreadsheetId:SheetName` (see "Google Sheets `tableId` forms").

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
table until `commit_change`. On an empty tab the record field names seed the header row.
For Google Sheets, `tableId` may be a URL, spreadsheet id, or
`spreadsheetId:gid` / `spreadsheetId:SheetName` (see "Google Sheets `tableId` forms").

Optionally, the append may carry a formatting plan (the same `formats`, `freezeRows`,
`freezeColumns`, and `columnWidths` fields as `preview_format_table`). When present it is
applied in the SAME commit, right after the rows land, so a fresh table is written and
styled in one preview -> commit instead of two.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 |
| `records` | array of objects (field name -> value) | 1 to 100 items |
| `formats` | array of format ops (see `preview_format_table`) | optional, at most 100 |
| `freezeRows` | integer | optional, 0 to 100 |
| `freezeColumns` | integer | optional, 0 to 100 |
| `columnWidths` | array of `{ column, pixels }` | optional, at most 100 |

Output shape: `{ "change": PendingChange, "requiresConfirmation": boolean }`

Diff shape (in `change.diff`): `{ "after": records }`, plus `"format": FormatPlan` when a
formatting plan was bundled.

Permission required: `write` (evaluated as the `append` action; when a format plan is
bundled, the `format` action is also checked so its confirmation requirement applies).

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

## `preview_format_table`

Purpose: create a pending formatting change and return it. Nothing is written until
`commit_change`. A plan is any mix of per-range cell formats, a header freeze, and column
widths; only the properties you set are changed (partial formatting). For Google Sheets,
`tableId` may be a URL, spreadsheet id, or `spreadsheetId:gid` / `spreadsheetId:SheetName`.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 |
| `formats` | array of `CellFormat` (below) | 0 to 100 items |
| `freezeRows` | number (optional) | 0 to 100 |
| `freezeColumns` | number (optional) | 0 to 100 |
| `columnWidths` | array of `{ column: string, pixels: number (2..2000) }` | 0 to 100 items |

At least one of `formats`, `freezeRows`, `freezeColumns`, or `columnWidths` must be set.

```ts
type CellFormat = {
  range: string;                                     // A1 range, e.g. "A1:D1", "B:B", "2:2"
  bold?: boolean; italic?: boolean;
  fontSize?: number;                                 // 1..400
  fontColor?: string;                                // "#rrggbb"
  backgroundColor?: string;                          // "#rrggbb"
  horizontalAlignment?: "LEFT" | "CENTER" | "RIGHT";
  numberFormat?: string;                             // pattern, e.g. "#,##0", "yyyy-mm-dd"
  numberFormatType?:                                 // inferred from the pattern when omitted
    "TEXT" | "NUMBER" | "PERCENT" | "CURRENCY" | "DATE" | "TIME" | "DATE_TIME" | "SCIENTIFIC";
  wrap?: boolean;
  border?: "none" | "all" | "outer" | "bottom";
};
```

Output shape: `{ "change": PendingChange, "requiresConfirmation": boolean }`

Diff shape (in `change.diff`): the plan itself (the `FormatPlan` above: `formats`,
`freezeRows`, `freezeColumns`, `columnWidths` with empty parts omitted).

Permission required: `write` (evaluated as the `format` action; a rule can list `format`
in `requireConfirmationFor` to require approval before commit). Only the Google Sheets
connector applies formatting; the mock connector returns an "does not support cell
formatting" error at commit.

House style: when laying out a fresh sheet or writing new data, freeze the header row,
make the header bold with a light neutral fill and a thin bottom border, give numeric and
date columns a consistent `numberFormat`, and set `columnWidths` so nothing is clipped.
When the sheet already has data or formatting, call `get_table_style` first and match it.

Example call:

```json
{
  "sourceId": "google-sheets:me_example_com",
  "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
  "formats": [
    { "range": "A1:E1", "bold": true, "backgroundColor": "#f3f4f6", "border": "bottom" },
    { "range": "D2:D1000", "horizontalAlignment": "RIGHT", "numberFormat": "#,##0" }
  ],
  "freezeRows": 1,
  "columnWidths": [{ "column": "A", "pixels": 220 }]
}
```

## `preview_create_spreadsheet`

Purpose: stage the creation of a brand-new spreadsheet on the connected account. Nothing
is created until `commit_change`; the commit's outcome carries `created` with the new
`spreadsheetId` and `url`. Source-level (there is no `tableId` yet).

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `title` | string | 1 to 200 characters |

Output shape: `{ "change": PendingChange, "requiresConfirmation": boolean }` (change type
`create_spreadsheet`; the change's `tableId` is empty).

Permission required: `write`, resolved against the **source-wide** rule (a rule with a null
`tableId`), since a create has no table yet.

## `preview_create_sheet`

Purpose: stage adding a new sheet tab to an existing spreadsheet. Nothing is created until
`commit_change`; the outcome's `created` carries the new tab's `sheetGid`. `tableId` is the
spreadsheet (URL or id).

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 (the spreadsheet) |
| `title` | string | 1 to 200 characters |

Output shape: `{ "change": PendingChange, "requiresConfirmation": boolean }` (change type
`create_sheet`).

Permission required: `write` on the spreadsheet.

## `preview_delete_sheet`

Purpose: stage deleting a sheet tab. Nothing is deleted until `commit_change`. Destructive.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | min length 1 |
| `tableId` | string | min length 1 (URL, `spreadsheetId:gid`, or `spreadsheetId:SheetName`) |

Output shape: `{ "change": PendingChange, "requiresConfirmation": boolean }` (change type
`delete_sheet`).

Permission required: `delete` (the `delete_records` permission, set by the **Bypass** access
preset). Auto-approve alone never authorizes this - if the source is not set to Bypass, the
preview is refused with a `Delete access denied` error.

## `commit_change`

Purpose: apply one or more previously previewed changes. This is the only tool that writes
to a table. Committing a `create_spreadsheet` or `create_sheet` change returns the new
resource in the outcome's `created` field.

Input schema (provide exactly one of the two forms):

| Field | Type | Bounds |
|---|---|---|
| `changeId` | string | single change; min length 1 |
| `changeIds` | array of strings | batch; 1 to 100 change ids, committed in order |

Output shape:
- Single (`changeId`): `{ "change": PendingChange, "records": TableRecord[] }` where
  `change.status` is `"committed"` and `records` are the written rows (updates return only
  the records that actually existed; appends return the new records with generated ids).
- Batch (`changeIds`): `{ "committed": CommitOutcome[] }`, one outcome per change in the
  order requested. All ids are checked to exist before any write, so a typo fails the batch
  before anything is committed; there is no cross-request rollback, so a failure partway
  through leaves the already-committed changes applied.

When an `append_records` change bundled a formatting plan and the rows committed but the
styling step failed, the outcome carries `"formatError": string` (the rows are committed,
so re-committing would duplicate them; retry the styling with `preview_format_table`).

Permission required: `write`, re-checked at commit time against fresh rules with the
same action evaluated at preview (`update` vs `bulk_update` vs `append`). Revoking write
access after preview blocks the commit.

Enforcement (see `docs/ipc.md`): auto-approve is on by default, so a change commits with
`decidedBy: "policy"` without a separate in-app approval - approving agent actions is the
agent harness's job. Only if the user has turned auto-approve off does a change created
with `requiresConfirmation: true` have to be approved in the desktop app first; changes
without a confirmation requirement always auto-approve at commit.

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
- The mock and Google Sheets connectors are functional; the `provider` connector is still
  a stub and calls against it fail with a routing error.
