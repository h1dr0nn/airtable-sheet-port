# Connectors

## Connector Abstraction

Connectors implement the provider-neutral `TableConnector` contract from
`@sheet-port/shared`:

- list sources
- list tables
- describe table
- read records (bounded)
- find records (text search)
- append records
- update records

The contract hides provider-specific tokens, endpoints, ranges, and response shapes from
agents.

## Connector Registry

`ConnectorRegistry` (`packages/core`) routes each call by source id. The registry is
constructed with a `ResolveSourceKind` lookup that reads the `sources.kind` column from
the shared SQLite database (via `SourceStore`), so adding a source row with a known kind
is enough to route it - no code changes in the registry. Unknown source ids and kinds
without a registered connector produce explicit errors.

The MCP sidecar (`apps/mcp-server/src/context.ts`) currently registers only the mock
connector.

## Mock Connector

`@sheet-port/mock-connector` (`packages/connectors/mock`) is fully SQLite-backed: table
schemas live in `mock_tables` and records in `mock_records`, shared with the desktop
UI. Committed changes made through MCP tools are immediately visible in the desktop
Tables screen, and everything persists across restarts of both processes.

Behavior:

- Schema discovery from the stored `FieldSchema[]` JSON.
- Bounded reads ordered by a stable `position` column; new records take
  `max(position) + 1`.
- Case-insensitive text search across all field values, capped at 100 results.
- Append generates `rec_<uuid>` ids; update shallow-merges patch fields and skips
  unknown record ids. Both run inside an immediate transaction.

Seed data (`packages/storage/seed.sql`): source `mock-source` ("Demo Workspace") with a
`customers` table and three records, plus a permission rule that requires confirmation
for every write action.

## Google Sheets Connector Plan

The Google Sheets connector will map a spreadsheet to a source and sheets/ranges to
tables. Planned work:

- Tauri OAuth flow using Google consent.
- Store refresh tokens in the OS keychain (service `sheet-port`, user `google_sheets`;
  the desktop `token_status` command already reads this entry).
- Discover spreadsheets through Drive picker or user-provided spreadsheet ids.
- Infer table headers from the first row or a configured header row.
- Map rows to records with stable generated row ids.
- Read and update only configured ranges.
- Preserve formulas unless a policy explicitly allows formula changes
  (`formula_change` is already a `ConfirmationAction`).

## Additional Provider Connector Plan

Additional provider connectors will map workspaces or bases to sources, tables to
tables, fields to schema fields, and records directly to internal records. Planned work:

- API key or OAuth setup stored in the OS keychain (user `provider`).
- Base and table discovery.
- Field type mapping to Airtable - Sheet Port field types.
- Pagination and rate-limit handling.
- Batch update and append operations.

## Google Sheets Range Mapping

Google Sheets does not have native row ids. Airtable - Sheet Port should create internal
record ids using a table id plus row number or a hidden configured id column. The
preferred production path is a stable id column because row-number ids shift when users
sort or insert rows.

## Additional Provider Mapping

Some providers already have stable record ids. Field metadata maps naturally into
`FieldSchema`, while linked records, attachments, collaborators, and formulas should
initially map to `unknown` until specific support is added.

## Current Limitations

- Google Sheets and additional provider connectors are skeleton packages that throw on
  use; their seeded source rows show as placeholders in the desktop UI.
- No rate limit or retry policy exists yet.
- The mock connector performs no schema validation on written field values.
