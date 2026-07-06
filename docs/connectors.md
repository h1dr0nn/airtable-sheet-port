# Connectors

## Connector Abstraction

Connectors implement the provider-neutral `TableConnector` trait defined in
`crates/sheet-port-core/src/connectors/mod.rs`:

- `kind` (the `SourceKind` the connector serves)
- `list_sources`
- `list_tables`
- `describe_table`
- `read_table` (bounded)
- `find_records` (text search)
- `append_records`
- `update_records`

Every method receives the shared `rusqlite::Connection`, so connectors that persist
state (like the mock connector) use the same database as the rest of the broker. The
trait hides provider-specific tokens, endpoints, ranges, and response shapes from
agents.

## Connector Registry

`ConnectorRegistry` (same module) routes each call by source id: it resolves the
`sources.kind` column through `sources::get_kind` and dispatches to the connector
registered for that kind, so adding a source row with a known kind is enough to route
it - no code changes in the registry. Registering a second connector for the same kind
replaces the first. Unknown source ids and kinds without a registered connector produce
explicit errors.

`ConnectorRegistry::with_default_connectors` (used by both the MCP sidecar and any
future desktop-side routing) currently registers only the mock connector; the Google
Sheets and provider connectors join once their auth lands.

## Mock Connector

`MockConnector` (`crates/sheet-port-core/src/connectors/mock.rs`, storage in
`mock_data.rs`) is fully SQLite-backed: table schemas live in `mock_tables` and records
in `mock_records`, shared with the desktop UI. Committed changes made through MCP tools
are immediately visible in the desktop Tables screen, and everything persists across
restarts of both processes.

Behavior:

- Schema discovery from the stored `FieldSchema[]` JSON.
- Bounded reads ordered by a stable `position` column; new records take
  `max(position) + 1`.
- Case-insensitive text search across all field values, capped at 100 results
  (`FIND_RECORDS_LIMIT`).
- Append generates `rec_<uuid>` ids; update shallow-merges patch fields and skips
  unknown record ids. Both run inside an immediate transaction.

Seed data (`crates/sheet-port-core/sql/seed.sql`): source `mock-source`
("Demo Workspace") with a `customers` table and three records, plus a permission rule
that requires confirmation for every write action.

## Stub Connectors

`GoogleSheetsConnector` and `ProviderConnector`
(`crates/sheet-port-core/src/connectors/google_sheets.rs` and `provider.rs`) implement
the trait but return explicit "not implemented" errors from every data method; they are
not registered in `with_default_connectors`. The seeded `google-placeholder` and
`provider-placeholder` source rows show as placeholders in the desktop UI and are not
returned by the MCP `list_sources` tool.

## Google Sheets Connector Plan

The Google Sheets connector will map a spreadsheet to a source and sheets/ranges to
tables. Planned work:

- Tauri OAuth flow using Google consent.
- Store refresh tokens in the OS keychain (service `sheet-port`, user `google_sheets`;
  the desktop `token_status` command already reads this entry through `vault.rs`).
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

- Google Sheets and additional provider connectors are stubs that return "not
  implemented" errors; their seeded source rows show as placeholders in the desktop UI.
- No rate limit or retry policy exists yet.
- The mock connector performs no schema validation on written field values.
