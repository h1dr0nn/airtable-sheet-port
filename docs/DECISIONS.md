# Decisions

## D1 - Auth through an Apps Script bridge instead of OAuth
The user is the only user and does not want Cloud Console setup. A web app deployed
from the user's own Apps Script project returns `ScriptApp.getOAuthToken()` when
the POST body carries the right secret. The token has the manifest's scopes
(`spreadsheets`, `drive.metadata.readonly`, `userinfo.email`). The existing REST
connector keeps working unchanged. Only the token source changes.

The manifest must enable the Sheets v4 and Drive v3 advanced services. Otherwise the
hidden default GCP project answers 403 "API has not been used". This was verified
live on 2026-09-24.

## D2 - A pool of bridges, one account each
Each bridge is stored as `{url, secret, deploymentId}` plus a cached access token in
the OS keychain. The account key is derived from the email the bridge reports, so
adding a second bridge for the same account replaces the first one instead of
duplicating it. The deployment id is parsed from the URL and is not typed by hand.

## D3 - Routing when `sourceId` is omitted
- One account: use it.
- Several accounts and a spreadsheet reference: probe each account's token with a
  cheap metadata GET, use the first that can open it, and remember the pair in
  `meta` (`google_route:{spreadsheetId}`).
- No reference: use the first account.

## D4 - No approval gate
Every write still goes through the staged-change pipeline, so each write keeps a
diff and an audit trail. By default the write tools stage and commit in one call and
return the diff so the agent can review its own work. `dryRun: true` only stages the
change; `commit_change` applies it later. The desktop approve/reject flow and the
auto-approve setting are removed. The `require_confirmation` column stays in SQLite
but is unused, because dropping it would need a table rebuild migration for no gain.

## D5 - Deleting a tab needs `confirm: true`
This replaces the "Bypass" preset requirement. New bridge sources get a source-wide
rule with read, write and delete allowed.

## D6 - Tool renames (breaking, hence 2.0.0)
`preview_update_records` becomes `update_records`, `preview_update_cells` becomes
`update_cells`, `preview_format_table` becomes `format_table`, and
`preview_create_spreadsheet`, `preview_create_sheet` and `preview_delete_sheet` lose
their `preview_` prefix. The old names no longer described what the tools do.

## D7 - Mock and stub connectors
The provider stub, which returned only TODO errors, is removed. The mock connector
is compiled only under the `mock` cargo feature, which tests and the e2e smoke
enable, so release binaries contain no mock. The frontend browser demo fixtures
are loaded only when `import.meta.env.DEV` is set, so production bundles do not
include them.

## D8 - No migration from OAuth accounts
No machine has a populated database (checked `%APPDATA%\sheet-port`), so old OAuth
keychain entries are not migrated. An entry that fails to parse as a bridge
credential is reported as "re-add this bridge".

## D9 - Headless `bridge` subcommand on the sidecar
`sheet-port-mcp bridge add|list|remove` lets a bridge be added without the desktop app
and drives the live smoke. The secret comes from `SHEET_PORT_BRIDGE_SECRET` or stdin,
never argv, so it stays out of the process list and shell history.

## D10 - A live test next to the offline suites
`pnpm test` stays offline and deterministic (mock connector, temp DB). `pnpm test:live`
exercises the release binary against a real bridge. It uses a temporary tab that it
creates and deletes, so the target spreadsheet ends unchanged. It passed against the
maintainer's bridge on 2026-09-24.

## D11 - Version 2.0.0
The tool names, the write output shape and the auth model all change incompatibly, so
this is a major bump. Files carry 2.0.0. The release tag keeps the repo's
`release-v<x.y.z>` convention (`release-v2.0.0`) and is created locally only. Pushing
it would start the release workflow.

## D12 - Known limits accepted after review
- **Routes do not expire.** A cached `google_route:{spreadsheetId}` stays in place
  until its bridge is removed. If an account later loses access, calls fail with
  Google's 403/404 until the caller passes `sourceId`. Probing again on every call
  would add a request to each call in multi-account setups for a rare case.
- **Account keys can collide.** An account key replaces every non-alphanumeric
  character with `_`, so `a.b@x.com` and `a_b@x.com` share a key and the second
  bridge replaces the first. This was accepted for a single-user app and keeps
  keychain entry names readable.
- **The live smoke writes the real keychain.** `pnpm test:live` writes the bridge
  credential to the real keychain entry for that account. It is the same credential
  the app stores, so the entry is left in place instead of being deleted afterwards.
- **`delete_sheet.confirm` is optional in the schema.** A missing value still returns
  the clear "needs confirm: true" error instead of a generic schema error.
