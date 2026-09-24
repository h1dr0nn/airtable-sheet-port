# Connectors

## Connector Abstraction

Connectors implement the provider-neutral `TableConnector` trait defined in
`crates/sheet-port-core/src/connectors/mod.rs`: `kind`, `list_sources`, `list_tables`,
`describe_table`, `read_table` (bounded), `find_records`, `append_records`,
`update_records`, plus the Google-specific capabilities (formula reads, raw cell reads
and writes, style reads, formatting, spreadsheet/tab creation and deletion) that return
an Unsupported error on connectors that lack them.

Every method receives the shared `rusqlite::Connection`. The trait hides tokens,
endpoints, ranges, and response shapes from agents.

## Connector Registry

`ConnectorRegistry` (same module) routes each call by source id: it resolves the
`sources.kind` column through `sources::get_kind` and dispatches to the connector
registered for that kind. Unknown source ids and kinds without a registered connector
produce explicit errors.

`ConnectorRegistry::with_default_connectors` registers the Google Sheets connector. With
the cargo feature `mock` it also registers the mock connector.

Source kinds: `google_sheets`, and `mock` under the feature. The former `provider` kind
and its stub connector were removed in 2.0.0.

## Google Sheets Connector

`crates/sheet-port-core/src/connectors/google_sheets.rs` maps a Google account to a
source, each spreadsheet to a table, and each tab to a table selected through the
`tableId` forms in `docs/mcp-tools.md`.

### Authentication (bridges)

The connector never sees a bridge secret. It asks the `google` module for an access
token for the source, which:

1. Loads the account's keychain entry (service `sheet-port`, user
   `google_sheets:{accountKey}`): `{bridgeUrl, secret, deploymentId}` plus the cached
   token and expiry.
2. Returns the cached token when it is valid for more than 60 more seconds.
3. Otherwise POSTs `{ "secret": ... }` to the bridge, stores the new token and expiry,
   and returns it.

The bridge itself is the Apps Script web app in `bridge/` (setup in `bridge/README.md`).
Because the bridge's hidden GCP project needs the Sheets and Drive APIs enabled, the
bridge manifest enables both advanced services; without them every API call returns 403.

### Account routing

When a tool omits `sourceId`, the core picks the account (see `docs/mcp-tools.md`,
"Source Routing"): single account; else the cached `meta.google_route:{spreadsheetId}`;
else probe each account with `GET spreadsheets/{id}?fields=spreadsheetId` and cache the
first that succeeds; else (nothing referenced) the first account.

### Mapping

- `list_tables`: one Drive listing (`mimeType = spreadsheet`, not trashed, ordered by
  name); each spreadsheet is a `TableRef` whose `tableId` is the spreadsheet id.
- `list_sheets`: the spreadsheet's tabs as `{ gid, title }`, left to right.
- Headers: row 1 of the tab; field types are inferred from the data.
- Records: every data row is a record with id `row_{n}` (n = 1-based sheet row, so data
  starts at `row_2`). Row ids shift when rows are inserted, deleted, or sorted.
- `read_cells`: raw A1-keyed cells for the tab or an optional A1 `range`, each row
  carrying its real sheet row number.
- Writes: `update_cells` uses USER_ENTERED semantics; appends go to the bottom of the
  tab (an empty tab gets a header row from the record keys); formatting, freeze and
  column widths are applied in the same commit as their change.
- All requests use the fixed Sheets/Drive endpoints; the `tableId` only supplies the
  spreadsheet id and tab selector (see `docs/security.md`).

## Mock Connector (tests only)

`MockConnector` (`crates/sheet-port-core/src/connectors/mock.rs`, storage in
`mock_data.rs`) is compiled only with the cargo feature `mock`, used by unit tests and
`pnpm test:e2e`. It is SQLite-backed (`mock_tables`, `mock_records`), ordered by a
stable `position` column, searches case-insensitively (capped at 100 results), generates
`rec_<uuid>` ids on append, and shallow-merges patches. It does not support formatting or
cell-level tools. Release builds do not contain it, and fresh databases contain no mock
data.

## Current Limitations

- Google Sheets is the only production connector.
- No rate-limit or retry policy exists yet; Google API errors surface as tool errors.
- Row-number record ids are not stable across structural edits by other people.
