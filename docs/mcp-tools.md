# MCP Tools

The Rust sidecar (`crates/sheet-port-mcp`) registers 18 tools. All input schemas are
provider-neutral (generated via `schemars`, with every bound enforced in `src/args.rs`);
none expose raw Google APIs, tokens, or the bridge secret. Every tool returns a single
text content block containing pretty-printed JSON with the shapes below. Every call
writes an audit event (actor `agent`).

| Tool | Kind | Input |
|---|---|---|
| `list_sources` | read | none |
| `list_tables` | read | `{ sourceId? }` |
| `list_sheets` | read | `{ sourceId?, tableId }` |
| `describe_table` | read | `{ sourceId?, tableId }` |
| `read_table` | read | `{ sourceId?, tableId, limit, offset }` |
| `read_formulas` | read | `{ sourceId?, tableId, limit, offset }` |
| `find_records` | read | `{ sourceId?, tableId, query }` |
| `read_cells` | read | `{ sourceId?, tableId, range?, limit, offset }` |
| `get_table_style` | read | `{ sourceId?, tableId, headerRow? }` |
| `update_records` | write | `{ sourceId?, tableId, patches, dryRun? }` |
| `append_records` | write | `{ sourceId?, tableId, records, formats?, freezeRows?, freezeColumns?, columnWidths?, validations?, conditionalFormats?, replaceIntersecting?, dryRun? }` |
| `update_cells` | write | `{ sourceId?, tableId, cells, dryRun? }` |
| `format_table` | write | `{ sourceId?, tableId, formats?, freezeRows?, freezeColumns?, columnWidths?, validations?, conditionalFormats?, replaceIntersecting?, dryRun? }` |
| `create_spreadsheet` | write | `{ sourceId?, title, dryRun? }` |
| `create_sheet` | write | `{ sourceId?, tableId, title, dryRun? }` |
| `delete_sheet` | delete | `{ sourceId?, tableId, confirm: true, dryRun? }` |
| `commit_change` | write | `{ changeId }` or `{ changeIds }` |
| `get_audit_log` | read | `{ limit? }` |

Every `inputSchema` in `tools/list` is fully inlined: no `$defs` or `$ref`, and every
field sits at the top level (the shared formatting fields of `format_table` and
`append_records` included) with a description. Array items describe their own fields
(`formats`, `validations`, `conditionalFormats`, `cells`, `patches`, ...), so clients that
drop `$defs` still show them.

Renamed in 2.0.0: `preview_update_records` -> `update_records`, `preview_update_cells`
-> `update_cells`, `preview_format_table` -> `format_table`,
`preview_create_spreadsheet` -> `create_spreadsheet`, `preview_create_sheet` ->
`create_sheet`, `preview_delete_sheet` -> `delete_sheet`. `list_sheets` is new. The
`requiresConfirmation` output field is gone.

Shared types (TypeScript notation; defined in `crates/sheet-port-core/src/types.rs` and
mirrored for the frontend in `packages/shared`):

```ts
type DataSource   = { id: string; kind: "google_sheets" | "mock"; name: string; status?: "connected" | "placeholder" | "error" };
type TableRef     = { sourceId: string; tableId: string; name: string };
type TableSchema  = { sourceId: string; tableId: string; name: string; fields: FieldSchema[]; locale?: string };
type FieldSchema  = { name: string; type: "string" | "number" | "boolean" | "date" | "email" | "enum" | "unknown"; required?: boolean; readonly?: boolean; enumValues?: string[] };
type TableRecord  = { id: string; fields: Record<string, unknown> };  // Google Sheets ids are "row_{sheetRow}"
type PendingChange = {
  id: string;                     // "chg_" + UUID
  sourceId: string;
  tableId: string;
  type: "append" | "update" | "update_cells" | "format" | "create_spreadsheet" | "create_sheet" | "delete_sheet" | "delete";
  createdAt: string;              // ISO timestamp
  status: "pending" | "approved" | "committed" | "rejected";
  diff: unknown;                  // see per-tool shapes below
  decidedAt?: string;
  decidedBy?: "user" | "policy";
  committedAt?: string;
};
type WriteResult = {              // every write tool and commit_change (lean shape)
  change: PendingChange;          // the committed change (status "committed"), or the staged one on dryRun
  committed: boolean;             // false only for dryRun, which returns just { change, committed }
  records?: TableRecord[];        // rows written (updates: only records that existed; appends: new rows); omitted when empty
  formatError?: string;           // bundled append+format: rows written, styling failed
  created?: { spreadsheetId?: string; sheetGid?: string; title?: string; url?: string }; // create_* changes
};
type AuditEvent   = { id: string; timestamp: string; actor: "user" | "agent" | "system"; action: string; sourceId?: string; tableId?: string; metadata?: Record<string, unknown> };
```

`kind: "mock"` exists only in builds with the cargo feature `mock` (tests and
`pnpm test:e2e`). Release builds serve Google Sheets sources only.

Examples below use one connected account, source `google-sheets:me_example_com`, and a
spreadsheet `1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms` with a `Customers` tab.

## Source Routing (`sourceId` is optional)

Every tool that takes `sourceId` accepts it as optional. One source exists per connected
Google account (`google-sheets:{accountKey}`, one bridge each). When `sourceId` is given
it is used as is. When it is omitted:

| Situation | Account used |
|---|---|
| Exactly one account connected | that account |
| Several accounts, and the call references a spreadsheet (`tableId`) | the first account whose bridge token can open it |
| Several accounts, nothing referenced (`list_tables`, `create_spreadsheet`) | the first account |
| No account connected | tool error (add a bridge in the desktop app) |

For the multi-account case the sidecar probes each account in order with
`GET {SHEETS_ENDPOINT}/{spreadsheetId}?fields=spreadsheetId`; the first that answers wins.
The pair is cached in the shared `meta` table under `google_route:{spreadsheetId}`, so
later calls for the same spreadsheet skip the probe. Pass `sourceId` explicitly to pin a
specific account (for example when two accounts can both open a file).

## Writes: commit by default, `dryRun` to stage

Every write still goes through the staged-change pipeline: a `pending_changes` row with
a diff is created, permissions are checked, and the commit is audited. There is no
approval step in between.

- **Default (`dryRun` omitted or `false`):** the tool stages the change and commits it in
  the same call. Output (lean shape): `{ "change": PendingChange, "committed": true,
  "records"?: TableRecord[], "formatError"?: string, "created"?: CreatedResource }`.
  `change` is the committed change (`status: "committed"`) and carries the diff; it
  appears once. `records` is omitted when empty, `formatError` and `created` when unset.
  Review the diff and correct with a follow-up write if needed.
- **`dryRun: true`:** the tool only stages. Output: `{ "change": PendingChange,
  "committed": false }`, with `change.status` `"pending"`. Apply it later with
  `commit_change`, or leave it; the user can discard a staged change from the desktop
  Changes screen.

## Google Sheets `tableId` forms

For a Google Sheets source, every table tool (`list_sheets`, `describe_table`,
`read_table`, `read_formulas`, `find_records`, `read_cells`, `get_table_style`,
`update_records`, `append_records`, `update_cells`, `format_table`, `create_sheet`,
`delete_sheet`) accepts the `tableId` in any of these forms. This lets an agent paste a
spreadsheet link the user shared and read the exact tab without extra lookups:

| Form | Example | Tab selected |
|---|---|---|
| Full Google Sheets URL | `https://docs.google.com/spreadsheets/d/1BxiMVs.../edit?gid=1234567#gid=1234567` | the tab whose gid matches |
| URL without a gid | `https://docs.google.com/spreadsheets/d/1BxiMVs.../edit` | the first tab |
| Bare spreadsheet id | `1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms` | the first tab |
| `spreadsheetId:gid` | `1BxiMVs...:1234567` | the tab whose gid matches |
| `spreadsheetId:SheetName` | `1BxiMVs...:Q3 Summary` | the tab with that exact title |

Resolution fetches the spreadsheet metadata once
(`GET {SHEETS_ENDPOINT}/{id}?fields=properties(title,locale,timeZone),sheets.properties(sheetId,title,index)`)
to map a gid to its tab title or validate a tab name; every subsequent read/write range is
qualified by the resolved title (e.g. `'Q3 Summary'!A1:ZZ`). A gid or name that does not
exist returns a `NotFound` tool error. Only the spreadsheet id and tab selector are ever
extracted from a URL - the HTTP host and endpoint are fixed to the Google Sheets API (see
`docs/security.md`, "Google Sheets URL Parsing"). Ids that do not look like a Google
document id (too short, or containing URL/host characters) are rejected before any
request is made.

`list_tables` returns one entry per spreadsheet (the tableId is the spreadsheet id,
resolving to the first tab). Use `list_sheets` to see the tabs, then pass a
tab-qualified form above.

## `list_sources`

Purpose: list connected data sources (read-only). One entry per connected Google account.

Input schema: none (empty object).

Output shape: `{ "sources": DataSource[] }`

Permission required: none beyond local MCP access (audited).

Example response:

```json
{
  "sources": [
    { "id": "google-sheets:me_example_com", "kind": "google_sheets", "name": "Google Sheets (me@example.com)", "status": "connected" }
  ]
}
```

## `list_tables`

Purpose: list the spreadsheets in a source (read-only). Each spreadsheet is one entry and
its `tableId` is the spreadsheet id.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 (see "Source Routing") |

Output shape: `{ "tables": TableRef[] }`

Permission required: `read` on the source.

Example response:

```json
{
  "tables": [
    { "sourceId": "google-sheets:me_example_com", "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms", "name": "CRM" }
  ]
}
```

## `list_sheets`

Purpose: list the tabs of one spreadsheet, left to right (read-only). Use the `gid` or
`title` to build a tab-qualified `tableId`.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 (any `tableId` form; the tab selector is ignored) |

Output shape: `{ "spreadsheetId": string, "locale"?: string, "timeZone"?: string, "sheets":
[{ "gid": string, "title": string }] }`. `locale` and `timeZone` come from the
spreadsheet's `properties.locale` / `properties.timeZone` (same metadata read, no extra
request) and are omitted only when the source does not report them.

The locale decides how `USER_ENTERED` values and formulas are parsed. Check it before
writing formulas or decimals: in comma-decimal locales (for example `vi_VN`, `de_DE`,
`fr_FR`, `pt_BR`, `es_ES`, `it_IT`, `ru_RU`, `id_ID`, `tr_TR`) formula arguments are
separated with `;` (`=COUNTIF(D10:D21;"Done")`, where `,` gives `#ERROR!`) and decimals use
a comma (`0,65`; `0.65` stays text). Percentages like `65%` and plain integers are safe in
every locale. The write tools never rewrite values; they are sent exactly as given.

Permission required: `read` on the source/spreadsheet.

Example call:

```json
{ "tableId": "https://docs.google.com/spreadsheets/d/1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms/edit" }
```

Example response:

```json
{
  "spreadsheetId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
  "locale": "vi_VN",
  "timeZone": "Asia/Ho_Chi_Minh",
  "sheets": [
    { "gid": "0", "title": "Customers" },
    { "gid": "1234567", "title": "Q3 Summary" }
  ]
}
```

## `describe_table`

Purpose: return the field schema of a tab, inferred from its header row (read-only).
`name` reflects the resolved tab (or the spreadsheet title when no tab is selected).

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 |

Output shape: `{ "schema": TableSchema }`. `schema.locale` is the spreadsheet locale
(see `list_sheets`), omitted when the source does not report it.

Permission required: `read` on the source/table.

Example response:

```json
{
  "schema": {
    "sourceId": "google-sheets:me_example_com",
    "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
    "name": "Customers",
    "fields": [
      { "name": "Name", "type": "string" },
      { "name": "Email", "type": "email" },
      { "name": "Seats", "type": "number" }
    ],
    "locale": "en_US"
  }
}
```

## `read_table`

Purpose: read bounded records from a tab (read-only). Row 1 is the header; each data row
is a record with id `row_{sheetRow}`.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 |
| `limit` | integer | 1 to 500, default 100 |
| `offset` | integer | >= 0, default 0 |

Output shape: `{ "records": TableRecord[] }` (in sheet row order)

Permission required: `read` on the source/table.

Example call:

```json
{ "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms", "limit": 2, "offset": 0 }
```

Example response:

```json
{
  "records": [
    { "id": "row_2", "fields": { "Name": "Aurora Labs", "Email": "ops@auroralabs.dev", "Seats": 24 } },
    { "id": "row_3", "fields": { "Name": "Basalt Co", "Email": "it@basalt.co", "Seats": 3 } }
  ]
}
```

## `read_formulas`

Purpose: read records like `read_table`, but with each cell's raw formula preserved - a
formula cell returns its `=...` text instead of the computed value (read-only). Use it
before overwriting cells that may be computed, so the formula logic is visible and not
clobbered. Same `tableId` forms and paging as `read_table`.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 |
| `limit` | integer | optional, 1 to 500 (default 100) |
| `offset` | integer | optional, >= 0 (default 0) |

Output shape: `{ "records": TableRecord[] }` where a field value is the cell's formula
string when it holds one, else its literal value.

Permission required: `read` on the source/table. Only the Google Sheets connector supports
this; others return an Unsupported error.

## `find_records`

Purpose: case-insensitive text search across all field values of a tab (read-only).

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 |
| `query` | string | 1 to 200 characters |

Output shape: `{ "records": TableRecord[] }` (at most 100 results)

Permission required: `read` on the source/table.

Example call:

```json
{ "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms", "query": "aurora" }
```

Example response:

```json
{
  "records": [
    { "id": "row_2", "fields": { "Name": "Aurora Labs", "Email": "ops@auroralabs.dev", "Seats": 24 } }
  ]
}
```

## `read_cells`

Purpose: raw coordinate-level read (read-only). Returns cells keyed by A1 column letter,
with the real 1-based sheet row number on each row and NO header/record interpretation.
The escape hatch for document-style sheets (merged banner rows, headers not on row 1,
totals rows, multiple blocks) where `read_table` sees fewer columns than the sheet
actually has.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 |
| `range` | string | optional A1 range within the tab, e.g. `B40:F60`; omitted = the whole tab from row 1 |
| `limit` | integer | optional, 1 to 500 (default 100); rows |
| `offset` | integer | optional, >= 0 (default 0); rows into the range |

Output shape:

```json
{
  "columns": ["B", "C", "D", "E", "F"],
  "rows": [
    { "row": 40, "cells": { "B": "Module", "C": "Task", "D": "Owner", "E": "Hours", "F": "Status" } },
    { "row": 41, "cells": { "B": "Auth", "C": "Bridge setup", "D": "Duc", "E": "6", "F": "done" } }
  ],
  "totalRows": 21
}
```

`row` is always the real sheet row, so a value read at `row: 41, "E"` is cell `E41` and
can be written back with `update_cells`. `totalRows` counts the rows in the range (or the
tab when `range` is omitted).

Permission required: `read` on the source/table.

## `get_table_style`

Purpose: read a tab's existing cell formatting so an agent can match it (read-only). It
returns the effective style of the header row and the row below it (the sample), plus the
frozen row/column counts and per-column pixel widths. Only properties actually set on a
cell are included, so the output stays compact.

The header is row 1 by default. On a document-style sheet whose table starts lower (banner
rows above a header on row 9, say) pass `headerRow: 9`; the sample is then row 10. The
read fetches only `A{headerRow}:ZZ{headerRow+1}`.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 |
| `headerRow` | integer | optional, 1-based, 1 to 1000000, default 1 |

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
  validation?: string;                               // "list", "checkbox", or another type in lowercase
};
type ColumnWidth = { column: string; pixels: number };
type TableStyle = {
  spreadsheetId: string;
  sheetTitle?: string;                               // omitted for the first tab
  frozenRowCount: number;
  frozenColumnCount: number;
  headerRow: number;                                 // 1-based row read as the header
  columnCount: number;                               // used (header) width
  header: CellStyle[];                               // row headerRow
  sample: CellStyle[];                               // row headerRow + 1 (first data row)
  columnWidths: ColumnWidth[];
  conditionalFormatCount: number;                    // conditional-format rules on the tab
};
```

Permission required: `read` on the source/table. Only the Google Sheets connector
implements this.

## `update_records`

Purpose: update fields of existing records (by `row_{n}` id) and return the diff.
Commits in the same call unless `dryRun` is set (see "Writes").

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 |
| `patches` | array of `{ recordId: string (min 1), fields: object }` | 1 to 100 items |
| `dryRun` | boolean | optional, default `false` |

Output shape: `{ "change": PendingChange, "committed": true, "records": TableRecord[] }`
(see "Writes"), or `{ "change": PendingChange, "committed": false }` with `dryRun: true`.

Diff shape (in `change.diff`): one entry per patch,
`[{ "recordId", "before": fields | null, "after": merged fields }]`. `before` is `null`
when the record id does not currently exist.

Permission required: `read` AND `write`. Read is checked first because the diff exposes
current record values. When `patches.length > 20` (`BULK_UPDATE_THRESHOLD`), the write
is evaluated as the `bulk_update` action instead of `update`.

Example call:

```json
{
  "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
  "patches": [
    { "recordId": "row_3", "fields": { "Seats": 10 } }
  ]
}
```

Example response:

```json
{
  "change": {
    "id": "chg_1f0d3c62-9a44-4b1e-9a1f-b1d2c3e4f5a6",
    "sourceId": "google-sheets:me_example_com",
    "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
    "type": "update",
    "createdAt": "2026-09-24T09:15:00.000Z",
    "status": "committed",
    "diff": [
      {
        "recordId": "row_3",
        "before": { "Name": "Basalt Co", "Email": "it@basalt.co", "Seats": 3 },
        "after": { "Name": "Basalt Co", "Email": "it@basalt.co", "Seats": 10 }
      }
    ],
    "decidedAt": "2026-09-24T09:15:00.410Z",
    "decidedBy": "policy",
    "committedAt": "2026-09-24T09:15:00.902Z"
  },
  "committed": true,
  "records": [
    { "id": "row_3", "fields": { "Name": "Basalt Co", "Email": "it@basalt.co", "Seats": 10 } }
  ]
}
```

## `append_records`

Purpose: append rows at the bottom of a tab and return the diff. On an empty tab the
record field names seed the header row. Commits in the same call unless `dryRun` is set.

Optionally, the append may carry a formatting plan (the same `formats`, `freezeRows`,
`freezeColumns`, `columnWidths`, `validations`, `conditionalFormats`, and
`replaceIntersecting` fields as `format_table`). It is applied in the SAME
commit, right after the rows land, so a fresh table is written and styled in one call.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 |
| `records` | array of objects (field name -> value) | 1 to 100 items |
| `formats` | array of `CellFormat` (see `format_table`) | optional, at most 100 |
| `freezeRows` | integer | optional, 0 to 100 |
| `freezeColumns` | integer | optional, 0 to 100 |
| `columnWidths` | array of `{ column, pixels }` | optional, at most 100 |
| `validations` | array of `Validation` (see `format_table`) | optional, at most 100 |
| `conditionalFormats` | array of `ConditionalFormat` (see `format_table`) | optional, at most 100 |
| `replaceIntersecting` | boolean | optional, default `false` (see `format_table`) |
| `dryRun` | boolean | optional, default `false` |

Output shape: as in "Writes". Diff shape (in `change.diff`): `{ "after": records }`, plus
`"format": FormatPlan` when a formatting plan was bundled.

If the rows are written but the bundled styling fails, the top-level `formatError` carries the
reason. The rows are already committed, so do not repeat the append; retry only the
styling with `format_table`.

Permission required: `write` (evaluated as the `append` action; a bundled format plan is
also checked as the `format` action).

Example call:

```json
{
  "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
  "records": [
    { "Name": "Dune Harbor", "Email": "hello@duneharbor.io", "Seats": 2 }
  ],
  "dryRun": true
}
```

Example response (`dryRun: true`):

```json
{
  "change": {
    "id": "chg_7a2b9c10-3d4e-4f56-8a9b-0c1d2e3f4a5b",
    "sourceId": "google-sheets:me_example_com",
    "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
    "type": "append",
    "createdAt": "2026-09-24T09:16:00.000Z",
    "status": "pending",
    "diff": {
      "after": [
        { "Name": "Dune Harbor", "Email": "hello@duneharbor.io", "Seats": 2 }
      ]
    }
  },
  "committed": false
}
```

## `update_cells`

Purpose: coordinate-level writes to individual cells by A1 reference (e.g. set `E48` to
`350h`). Values are written with USER_ENTERED semantics - numbers parse as numbers, a
leading `=` becomes a live formula, anything else is text - matching what typing into the
sheet would do. Use whenever the record tools cannot address a cell (pair it with
`read_cells`, whose row numbers are real sheet rows). Commits in the same call unless
`dryRun` is set.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 |
| `cells` | array of `{ cell, value }` | 1 to 100 items; `cell` is an A1 ref within A..ZZ, row >= 1; `value` at most 50000 chars |
| `dryRun` | boolean | optional, default `false` |

Output shape: as in "Writes" (change type `update_cells`; the diff lists each
`{ cell, value }`; `records` is omitted).

Values are sent exactly as given and never rewritten, so they must match the
spreadsheet's locale: in a comma-decimal locale such as `vi_VN`, write `0,65` (not
`0.65`, which stays text) and separate formula arguments with `;`. See `list_sheets`.

Permission required: `write` (evaluated as the `update` action; more than 20 cells
escalates to `bulk_update` like a large record update).

## `format_table`

Purpose: apply formatting to a tab. A plan is any mix of per-range cell formats, a
freeze, column widths, native data validations (dropdowns and checkboxes), and
conditional formats; only the properties you set are changed (partial formatting).
Commits in the same call unless `dryRun` is set.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 |
| `formats` | array of `CellFormat` (below) | 0 to 100 items |
| `freezeRows` | number (optional) | 0 to 100 |
| `freezeColumns` | number (optional) | 0 to 100 |
| `columnWidths` | array of `{ column: string, pixels: number (2..2000) }` | 0 to 100 items |
| `validations` | array of `Validation` (below) | 0 to 100 items |
| `conditionalFormats` | array of `ConditionalFormat` (below) | 0 to 100 items |
| `replaceIntersecting` | boolean | optional, default `false`; widens the rule replace (below) |
| `dryRun` | boolean | optional, default `false` |

At least one of `formats`, `freezeRows`, `freezeColumns`, `columnWidths`, `validations`,
or `conditionalFormats` must be set.

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

// Native data validation (setDataValidation). Setting a rule replaces the
// range's previous validation.
type Validation = {
  range: string;                                     // A1 range, e.g. "D2:D100", "E:E"
  type: "list" | "checkbox";
  values?: string[];                                 // list only, required: 1..100 non-empty strings
  strict?: boolean;                                  // default true; false only warns
  showDropdown?: boolean;                            // list only, default true (showCustomUi)
};

// Conditional format (addConditionalFormatRule with a BooleanRule).
type ConditionalFormat = {
  range: string;                                     // A1 range
  when: {                                            // exactly one key
    textEq?: string;                                 // TEXT_EQ
    textContains?: string;                           // TEXT_CONTAINS
    numberGt?: number;                               // NUMBER_GREATER
    numberLt?: number;                               // NUMBER_LESS
    numberBetween?: [number, number];                // NUMBER_BETWEEN, low <= high
    blank?: true;                                    // BLANK
    notBlank?: true;                                 // NOT_BLANK
    formula?: string;                                // CUSTOM_FORMULA, starts with "="
  };
  backgroundColor?: string;                          // "#rrggbb"
  fontColor?: string;                                // "#rrggbb"
  bold?: boolean;                                    // at least one of the three is required
};
```

A `list` validation gives the native dropdown chip; `checkbox` gives native checkboxes
(`strict` rejects other values). `values` and `showDropdown` are rejected on a checkbox.

Conditional formats **replace** rather than pile up: before adding, the commit reads the
tab's rules (one `spreadsheets.get` with
`fields=sheets(properties.sheetId,conditionalFormats)`, only when the plan has rules) and
deletes every existing rule whose range set is exactly one of the new rules' ranges (the
same grid range), highest index first. The new rules are then inserted at the top in the
order given (earlier = higher priority). Rules on any other range are kept, including
ones that merely overlap: adding a whole-row rule on `B10:I21` leaves the status and
priority rules on `D10:D21` and `E10:E21` in place. So calling `format_table` again with
the same ranges updates the rules instead of duplicating them.

With `replaceIntersecting: true` the commit instead deletes every existing rule with a
range that intersects any of the new rules' ranges (the 2.1.0 behavior), which is the
way to clear a block's old color rules before restyling it. The flag appears in the diff
only when true. Number values in
`when` are sent with the spreadsheet's decimal mark (`0,5` in a comma-decimal locale).
A `formula` is sent as written, so write it in the spreadsheet's locale syntax (`;`
separators in comma-decimal locales).

Output shape: as in "Writes". Diff shape (in `change.diff`): the plan itself (the
`FormatPlan`: `formats`, `freezeRows`, `freezeColumns`, `columnWidths`, `validations`,
`conditionalFormats`, `replaceIntersecting` with empty parts and a false flag omitted; validations show their effective `strict`
and, for lists, `showDropdown`).

Permission required: `write` (evaluated as the `format` action). Only the Google Sheets
connector applies formatting.

House style: when laying out a fresh sheet or writing new data, freeze the header row,
make the header bold with a light neutral fill and a thin bottom border, give numeric and
date columns a consistent `numberFormat`, and set `columnWidths` so nothing is clipped.
When the sheet already has data or formatting, call `get_table_style` first and match it.

Example call:

```json
{
  "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
  "formats": [
    { "range": "A1:C1", "bold": true, "backgroundColor": "#f3f4f6", "border": "bottom" },
    { "range": "C2:C1000", "horizontalAlignment": "RIGHT", "numberFormat": "#,##0" }
  ],
  "freezeRows": 1,
  "columnWidths": [{ "column": "A", "pixels": 220 }]
}
```

Example call (status dropdown, done checkbox, and colored statuses):

```json
{
  "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms:Tasks",
  "validations": [
    { "range": "D2:D200", "type": "list", "values": ["Todo", "Doing", "Done"] },
    { "range": "E2:E200", "type": "checkbox" }
  ],
  "conditionalFormats": [
    { "range": "D2:D200", "when": { "textEq": "Done" }, "backgroundColor": "#d1fae5", "fontColor": "#065f46" },
    { "range": "D2:D200", "when": { "textEq": "Doing" }, "backgroundColor": "#fef3c7" },
    { "range": "F2:F200", "when": { "numberLt": 0.5 }, "fontColor": "#b91c1c", "bold": true }
  ]
}
```

Resulting diff (`change.diff`):

```json
{
  "validations": [
    { "range": "D2:D200", "type": "list", "values": ["Todo", "Doing", "Done"], "strict": true, "showDropdown": true },
    { "range": "E2:E200", "type": "checkbox", "strict": true }
  ],
  "conditionalFormats": [
    { "range": "D2:D200", "when": { "textEq": "Done" }, "backgroundColor": "#d1fae5", "fontColor": "#065f46" },
    { "range": "D2:D200", "when": { "textEq": "Doing" }, "backgroundColor": "#fef3c7" },
    { "range": "F2:F200", "when": { "numberLt": 0.5 }, "fontColor": "#b91c1c", "bold": true }
  ]
}
```

## `create_spreadsheet`

Purpose: create a brand-new spreadsheet on the account. Source-level (there is no
`tableId` yet); when `sourceId` is omitted with several accounts connected, the first
account is used. On commit, the top-level `created` carries the new `spreadsheetId` and `url`.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `title` | string | 1 to 200 characters |
| `dryRun` | boolean | optional, default `false` |

Output shape: as in "Writes" (change type `create_spreadsheet`; the change's `tableId` is
empty).

Permission required: `write`, resolved against the **source-wide** rule (a rule with a
null `tableId`), since a create has no table yet.

## `create_sheet`

Purpose: add a new tab to an existing spreadsheet. On commit, the top-level `created` carries
the new tab's `sheetGid`. `tableId` is the spreadsheet (URL or id).

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 (the spreadsheet) |
| `title` | string | 1 to 200 characters |
| `dryRun` | boolean | optional, default `false` |

Output shape: as in "Writes" (change type `create_sheet`).

Permission required: `write` on the spreadsheet.

## `delete_sheet`

Purpose: delete a sheet tab. Destructive and not undoable from the broker.

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `sourceId` | string | optional, min length 1 |
| `tableId` | string | min length 1 (URL with gid, `spreadsheetId:gid`, or `spreadsheetId:SheetName`) |
| `confirm` | boolean | required, must be `true`; anything else is an `InvalidInput` error |
| `dryRun` | boolean | optional, default `false` |

Output shape: as in "Writes" (change type `delete_sheet`).

Permission required: `delete` (the `deleteRecords` permission). A source without delete
access refuses the call with a `Delete access denied` error. New bridge sources start
with delete allowed; turn it off in the desktop permission rules to block this tool.

## `commit_change`

Purpose: apply one or more changes staged with `dryRun: true`. Committing a
`create_spreadsheet` or `create_sheet` change returns the new resource in the top-level
`created` field.

Input schema (provide exactly one of the two forms):

| Field | Type | Bounds |
|---|---|---|
| `changeId` | string | single change; min length 1 |
| `changeIds` | array of strings | batch; 1 to 100 change ids, committed in order |

Output shape:
- Single (`changeId`): the same lean shape as a direct write - `{ "change":
  PendingChange, "committed": true, "records"?, "formatError"?, "created"? }` with
  `change.status` `"committed"` and `records` omitted when empty.
- Batch (`changeIds`): `{ "committed": [lean outcome, ...] }`, one per change in the
  order requested. All ids are checked to exist before any write, so a typo fails the batch
  before anything is committed; there is no cross-request rollback, so a failure partway
  through leaves the already-committed changes applied.

Permission required: `write` (or `delete` for `delete_sheet`), re-checked at commit time
against fresh rules with the same action evaluated when the change was staged (`update`
vs `bulk_update` vs `append` vs `format`). Revoking write access after a dry run blocks
the commit.

Example call:

```json
{ "changeId": "chg_7a2b9c10-3d4e-4f56-8a9b-0c1d2e3f4a5b" }
```

Example response:

```json
{
  "change": {
    "id": "chg_7a2b9c10-3d4e-4f56-8a9b-0c1d2e3f4a5b",
    "sourceId": "google-sheets:me_example_com",
    "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
    "type": "append",
    "createdAt": "2026-09-24T09:16:00.000Z",
    "status": "committed",
    "diff": {
      "after": [
        { "Name": "Dune Harbor", "Email": "hello@duneharbor.io", "Seats": 2 }
      ]
    },
    "decidedAt": "2026-09-24T09:17:04.100Z",
    "decidedBy": "policy",
    "committedAt": "2026-09-24T09:17:04.620Z"
  },
  "committed": true,
  "records": [
    { "id": "row_5", "fields": { "Name": "Dune Harbor", "Email": "hello@duneharbor.io", "Seats": 2 } }
  ]
}
```

Error cases (returned as tool errors with these messages):

| Condition | Error |
|---|---|
| Change id does not exist | `Unknown change <changeId>` |
| Discarded by the user in the desktop app | `Change <changeId> was rejected in the desktop app and cannot be committed` |
| Already committed | `Change <changeId> is already committed` |
| Write permission revoked since staging | `Write access denied for <sourceId>/<tableId>` |

## `get_audit_log`

Purpose: return recent audit events, newest first (read-only).

Input schema:

| Field | Type | Bounds |
|---|---|---|
| `limit` | integer | 1 to 500, default 100 |

Output shape: `{ "events": AuditEvent[] }`

Permission required: none beyond local MCP access (the call itself is audited).

Example response:

```json
{
  "events": [
    {
      "id": "evt_0b9c8d7e-6f5a-4b3c-2d1e-0f9a8b7c6d5e",
      "timestamp": "2026-09-24T09:15:00.905Z",
      "actor": "agent",
      "action": "commit_change",
      "sourceId": "google-sheets:me_example_com",
      "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
      "metadata": { "changeId": "chg_1f0d3c62-9a44-4b1e-9a1f-b1d2c3e4f5a6", "recordCount": 1 }
    },
    {
      "id": "evt_2d3e4f5a-6b7c-8d9e-0f1a-2b3c4d5e6f7a",
      "timestamp": "2026-09-24T09:15:00.005Z",
      "actor": "agent",
      "action": "update_records",
      "sourceId": "google-sheets:me_example_com",
      "tableId": "1BxiMVs0XRA5nFMdKvBdBZjgmUUqptlbs74OgvE2upms",
      "metadata": { "changeId": "chg_1f0d3c62-9a44-4b1e-9a1f-b1d2c3e4f5a6", "patchCount": 1 }
    }
  ]
}
```

## Current Limitations

- Record-level delete (`delete` change type) is typed but not exposed as a tool; only
  whole tabs can be deleted (`delete_sheet`).
- Record ids are sheet row numbers (`row_{n}`), so they shift when rows are inserted,
  deleted, or sorted between a read and a write. Re-read before writing to a sheet other
  people edit.
- Only Google Sheets sources exist in release builds.
