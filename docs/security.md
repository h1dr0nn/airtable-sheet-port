# Security

## Trust Model (read this first)

Airtable - Sheet Port assumes AI agents may be over-permissioned, prompt-injected, or
confused by untrusted spreadsheet content. It reduces blast radius by exposing typed,
bounded local tools instead of Google credentials or broad execution primitives, and by
recording every write as a staged change with a diff in a persistent audit log.

What it does **not** do, since 2.0.0:

- **There is no human-in-the-loop inside the broker.** Write tools stage and commit in
  one call by default. There is no desktop approve step, no auto-approve setting, and no
  per-action confirmation list. The only broker-side boundaries are the permission rules
  (read / write / delete per source or spreadsheet) and the input bounds.
- **Approvals are the agent harness's job.** If you want a person to confirm writes, do
  it where the agent runs: the MCP client's tool-permission prompts (for example, require
  approval for `update_records`, `update_cells`, `format_table`, `create_*`,
  `delete_sheet`, `commit_change`), or instruct the agent to call write tools with
  `dryRun: true` and commit only after review. The desktop Changes screen is a history
  view, not a gate.
- **Anyone holding a bridge's secret can mint tokens.** The Apps Script bridge returns a
  one-hour OAuth access token for the deploying Google account to any caller that
  presents the `/exec` URL and the secret. That token is not limited by this app's
  permission rules. Protect the secret like a password (see "Bridge Secrets").

Use the permission rules to deny what an agent should never do: turn `write` off for
read-only sources, and turn `delete` off to block `delete_sheet`. New bridge sources
start with read, write and delete all allowed.

## Single-Language Trust Surface

The entire broker path is Rust: the Tauri desktop backend, the MCP sidecar
(`crates/sheet-port-mcp`), and the shared core (`crates/sheet-port-core`) that
implements every check described below. Consequences:

- No npm packages execute inside the broker. The Node/npm supply chain is limited to
  the React frontend (UI rendering only), which never touches tokens or the database
  directly - it talks to the Rust backend through the typed commands in `docs/ipc.md`.
- Tokens and bridge secrets never leave Rust. The keychain vault, the Google module, the
  permission engine, and the connectors live in one crate, compiled into both processes.
- SQLite is compiled in (rusqlite `bundled`), so the broker does not depend on a
  system SQLite either.
- Enforcement logic exists exactly once: the desktop app and the sidecar cannot drift
  apart, because both call the same `sheet-port-core` functions.
- The mock connector is compiled only under the cargo feature `mock` (tests and
  `pnpm test:e2e`); release builds do not contain it. The browser demo fixtures in the
  frontend are loaded only in Vite dev mode.

## Google Access via Bridges

There is no OAuth client, Cloud Console project, client id or client secret. Each Google
account is connected through an Apps Script web app the user deploys on that account
(`bridge/`, setup in `bridge/README.md`):

- The bridge runs as the deploying user (`executeAs: USER_DEPLOYING`) with scopes
  `spreadsheets`, `drive.metadata.readonly`, and `userinfo.email`, and is reachable
  anonymously (`access: ANYONE_ANONYMOUS`). The secret is the only thing that gates it.
- `doPost` compares the posted secret with the one `setup()` stored in Script
  Properties and, on a match, returns `ScriptApp.getOAuthToken()`, the account email, and
  `expiresInSec: 3000`. A wrong secret returns `unauthorized` and no token.
- The broker requests a token only from bridge URLs the user pasted into the desktop app.
  Agents cannot add, change, or read bridges.

## Bridge Secrets and Token Handling

- Each bridge is stored in the OS keychain under service `sheet-port`, user
  `google_sheets:{accountKey}` (accountKey = the sanitized email the bridge reports). The
  entry holds `{bridgeUrl, secret, deploymentId}` plus the cached access token and its
  expiry. Nothing secret is written to the SQLite database.
- One source per account (`google-sheets:{accountKey}`); adding a bridge for the same
  email replaces the old entry.
- The cached token is reused until it is within 60 seconds of expiry, then re-fetched
  from the bridge. The MCP sidecar reads the keychain directly, so it works without the
  desktop app running.
- Raw tokens and secrets never cross Tauri IPC and are never exposed to agents or the
  frontend. `google_list_accounts` returns only `sourceId`, `email`, `deploymentId`, and
  `bridgeUrl`; `token_status` returns only booleans.
- Keychain errors are logged to stderr and reported as "absent" rather than leaking
  details to the UI.

What a leaked secret means, and what to do:

| Leaked | Impact | Response |
|---|---|---|
| `/exec` URL only | none (the secret is required) | nothing |
| URL + secret | anyone can mint 1-hour tokens for the account, bypassing this app | rotate the secret (delete the `SECRET` script property, run `setup`, re-add the bridge) |
| an access token | full scope access until it expires (at most about an hour) | revoke the script at myaccount.google.com > Security > Third-party access |

Tokens are also held in the broker process memory while in use; any local process that
can read the user's keychain can read the bridge entries.

## Staged Changes (No Approval Gate)

Writes still go through the pending-change pipeline in
`crates/sheet-port-core/src/changes.rs`, which is what gives diffs, audit and atomicity:

1. A write tool evaluates the permission rule (fresh, never cached), builds the diff, and
   inserts a `pending_changes` row.
2. Unless `dryRun: true`, the same call commits it: the rule is re-checked, the connector
   writes, and the row moves to `committed`. The output carries `{ change, committed:
   true, outcome }` so the agent sees the diff it just applied.
3. With `dryRun: true` the row stays `pending`. `commit_change` applies it later; the
   user can discard it in the desktop Changes screen (`reject_change`), after which
   `commit_change` refuses it.

All status transitions are atomic guarded UPDATEs (`... WHERE id = ? AND status = ?`),
so concurrent actors cannot race a change into an invalid state or apply it twice.

## Permission Re-Check at Commit

Permission rules may change between a dry run and its commit. The commit path re-reads
the rules and re-evaluates the exact action evaluated when the change was staged: an
update with more than `BULK_UPDATE_THRESHOLD` (20) patches or cells is re-checked as
`bulk_update`, not plain `update`; `delete_sheet` is re-checked against `delete`.
Revoking `write` or `delete` in the desktop app therefore blocks commits of staged
changes.

## Read Gate on Update Diffs

`update_records` requires `read` in addition to `write`, because the returned diff
contains the current (before) values of the patched records. A write-only rule cannot be
used to exfiltrate table contents through diffs.

## Destructive Operations

`delete_sheet` requires the `delete` permission and an explicit `confirm: true` in the
input; a missing or false `confirm` is an `InvalidInput` error before anything is staged.
`confirm` is a guard against accidental calls, not a human approval: the agent sets it.

## Google Sheets URL Parsing (No SSRF)

A Google Sheets `tableId` may be a full spreadsheet URL, a bare spreadsheet id, or
`spreadsheetId:gid` / `spreadsheetId:SheetName` (see `docs/mcp-tools.md`). Parsing this
input is strictly an extraction step and never chooses where a request goes:

- The parser only pulls the spreadsheet id and a tab selector (a numeric `gid` or a sheet
  title) out of the input. It never derives an HTTP host, port, path, or endpoint from it.
  Every Sheets/Drive call is built from the fixed `SHEETS_ENDPOINT`
  (`https://sheets.googleapis.com/v4/spreadsheets`) and the Drive endpoint, with the id
  and range pushed as percent-encoded path segments via the `url` crate. There is no code
  path where a tableId (or the host in a pasted URL) can redirect a request to another
  origin, so a malicious link cannot be used for SSRF or to reach an internal service.
- URLs are only accepted when the host is a Google document host (`docs.google.com` /
  `drive.google.com`); any other host is rejected rather than followed. The host is used
  solely as a validation gate, not for routing.
- The extracted spreadsheet id must look like a Google document id (URL-safe base64
  alphabet `A-Za-z0-9_-`, and long enough). Ids containing `/`, `:`, spaces, or other URL
  syntax, and short/junk strings, are rejected before any request is built.
- A `gid` or sheet name that does not exist in the spreadsheet surfaces as a `NotFound`
  tool error after a single metadata lookup against the fixed endpoint.
- Bridge URLs come only from the desktop user, never from tool input.

Account routing (when `sourceId` is omitted) probes each connected account against the
same fixed endpoint (`GET spreadsheets/{id}?fields=spreadsheetId`) and caches the winner
in `meta` as `google_route:{spreadsheetId}`. Routing only chooses among the user's own
accounts; it cannot reach a host other than the Google APIs.

## Desktop Hardening (CSP + Capabilities)

- CSP (`tauri.conf.json`): `default-src 'self'; script-src 'self'; style-src 'self'
  'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self' ipc:
  http://ipc.localhost`. No remote origins, no `unsafe-eval`, no external script
  loading. Bridge calls are made from Rust, not from the webview.
- Capabilities (`src-tauri/capabilities/default.json`): the main window gets only
  `core:default` plus the window permissions needed by the custom titlebar
  (minimize, toggle-maximize, close, start-dragging) and the plugins in use. No fs,
  shell, http, or dialog permissions are granted.
- The window runs with `decorations: false` and a custom titlebar; all frontend/backend
  interaction goes through the typed commands in `docs/ipc.md`.

## Local MCP Attack Surface

The MCP server runs locally. The default transport is stdio (spawned by the agent's MCP
client), which exposes no network surface at all.

An optional loopback HTTP transport (rmcp streamable-http) can be selected in settings.
When enabled it binds STRICTLY to `127.0.0.1:{port}` and is never exposed externally:

- The listener address is built from `Ipv4Addr::LOCALHOST` in
  `crates/sheet-port-mcp/src/http.rs`; there is no code path that binds `0.0.0.0` or a
  routable interface. This is a hard rule.
- The transport keeps rmcp's default loopback-only `allowed_hosts`
  (`localhost`, `127.0.0.1`, `::1`), which also defends against DNS-rebinding attacks
  from a browser on the same machine.
- The port is validated to `1024-65535` (`set_mcp_port`); privileged ports are rejected.
  A port already in use makes the sidecar log the conflict to stderr and exit non-zero
  rather than falling back to an unexpected address.
- The HTTP transport has no authentication of its own: any local process that can reach
  the port can call the tools. Prefer stdio unless a client needs HTTP.
- For the HTTP transport the desktop app can manage the sidecar as a child process
  (`mcp_server_start` / `mcp_server_stop`, see `docs/ipc.md`) and kills it on app exit.

Tool input schemas are strict, bounded, and provider-neutral, enforced in
`crates/sheet-port-mcp/src/args.rs`: page limits 1-500, query max 200 chars, at most 100
patches/records/cells per change. Out-of-range input surfaces as a clear tool error,
never as a raw schema failure.

## Tool Allowlist

Exposed (18 tools, `docs/mcp-tools.md`):

- list sources, spreadsheets and tabs; describe schema
- bounded reads, formula reads, raw cell reads, text search, style reads
- record updates, appends, cell writes, formatting, spreadsheet/tab creation, tab
  deletion (each staged as a change, committed unless `dryRun`)
- commit staged changes
- read the audit log

Not exposed, by design:

- shell command execution
- arbitrary JavaScript execution
- raw Google API calls, tokens, bridge URLs or secrets
- SQL execution
- record-level deletes

## Permission Checks

Read tools require `read`. Write tools and commits require `write`. Updates touching
more than 20 records or cells are evaluated as `bulk_update`. `delete_sheet` requires
`delete` (`deleteRecords`). Rule precedence: an exact `(sourceId, tableId)` rule wins
over a source-wide rule. Creating a spreadsheet is checked against the source-wide rule.

## Audit Log

Audit events persist in the shared SQLite `audit_events` table with timestamp, actor
(`user` / `agent` / `system`), action, source/table scope, and JSON metadata. Both
processes write to it: the sidecar records every tool call (including staging and
commits), the desktop records permission edits, bridge changes, and discards. Events
survive restarts of either process. The audit log records what happened; it does not
prevent anything.

## Current Limitations

- No human approval gate in the broker (by design, see "Trust Model").
- A bridge secret is a bearer credential for the whole Google account scope; the app
  cannot narrow what a token minted outside it can do.
- The SQLite database is unencrypted at rest; any local process running as the same OS
  user can read or modify it (including permission rules and staged changes).
- Record ids are sheet row numbers, so a concurrent edit by a person can shift which row
  an agent's `row_{n}` update lands on.
- For the stdio transport the desktop app does not manage the sidecar lifecycle; the
  agent's MCP client spawns it.
