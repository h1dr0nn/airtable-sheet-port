# Security

## Threat Model

Airtable - Sheet Port assumes AI agents may be over-permissioned, prompt-injected, or
confused by untrusted spreadsheet content. The app reduces blast radius by exposing
typed local tools instead of provider credentials or broad execution primitives, and by
forcing every write through a persisted preview -> approve -> commit pipeline.

## Single-Language Trust Surface

The entire broker path is Rust: the Tauri desktop backend, the MCP sidecar
(`crates/sheet-port-mcp`), and the shared core (`crates/sheet-port-core`) that
implements every check described below. Consequences:

- No npm packages execute inside the broker. The Node/npm supply chain is limited to
  the React frontend (UI rendering only), which never touches tokens or the database
  directly - it talks to the Rust backend through the typed commands in `docs/ipc.md`.
- Tokens never leave Rust. The keychain vault (`vault.rs`), the permission engine, and
  the connectors live in one crate, compiled into both processes.
- SQLite is compiled in (rusqlite `bundled`), so the broker does not depend on a
  system SQLite either.
- Enforcement logic exists exactly once: the desktop app and the sidecar cannot drift
  apart, because both call the same `sheet-port-core` functions.

## Confirmation Enforcement (Cross-Process)

Confirmation is enforced, not advisory. The desktop app and the MCP sidecar share one
SQLite database (see `docs/ipc.md`, the canonical contract), and the sidecar reads fresh
state on every call, so a desktop decision applies immediately without any direct IPC.

Mechanism (implemented in `crates/sheet-port-core/src/changes.rs`):

1. `preview_update_records` / `append_records` insert a `pending_changes` row with
   `requires_confirmation` snapshotted from the matching permission rule
   (`requireConfirmationFor` containing the evaluated action). The flag is returned to
   the agent as `requiresConfirmation` so it knows to ask the user.
2. `commit_change` re-reads the row and refuses to proceed when:
   - the change is unknown,
   - status is `rejected` (the user declined in the desktop app),
   - status is `committed` (already applied),
   - `requires_confirmation` is set, status is not `approved`, and auto-approve is off
     (the agent is told to ask the user to approve in the desktop app).
3. A `pending` change auto-approves with `decided_by = 'policy'` when
   `requires_confirmation` is false, or when it is true but auto-approve is on (the
   default; see below).

All status transitions are atomic guarded UPDATEs, so concurrent actors cannot race a
change into an invalid state or apply it twice:

- Desktop approve/reject: `UPDATE pending_changes SET status = ?, decided_at = ?,
  decided_by = 'user' WHERE id = ? AND status = 'pending'`; zero affected rows is
  reported as an error with the actual current status.
- Sidecar policy approval: the same guarded `pending -> approved` transition with
  `decided_by = 'policy'`.
- Commit finalization: `UPDATE ... SET status = 'committed' WHERE id = ? AND
  status = 'approved'`; a missed guard aborts the commit with an error.

## Auto-Approve (On by Default)

Auto-approve bypasses the broker's own human confirmation gate for agent
writes. Approving agent actions is the agent harness's responsibility, so the
broker does not gate commits a second time by default. The setting is stored in
the shared `meta` table under `auto_approve_writes` and toggled from the desktop
app (`set_auto_approve`, see `docs/ipc.md`).

- Default on: the meta key is absent (or `"1"`). The commit path reads the flag
  fresh (never cached) and treats a `requires_confirmation` change as
  policy-approved (`decided_by = 'policy'`) instead of refusing it. The change
  still passes the permission re-check and is still audited and committed
  atomically - only the broker's own human approval step is skipped.
- When off (value `"0"`): commit blocks every unapproved `requires_confirmation`
  change exactly as described above, restoring the broker's in-app
  human-in-the-loop confirmation gate.

With auto-approve on, an over-permissioned or prompt-injected agent can commit
writes without a person approving each one inside this app - the permission
rules (read/write/delete per source) remain the enforced boundary, and the
staged preview -> commit trail is kept for the audit log and the desktop
history view. Turning the setting off is recorded in the audit log
(`settings_updated`); `reset_settings` returns it to the default on state.

## Permission Re-Check at Commit

Permission rules may change between preview and commit. The commit path re-reads the
rules (they are never cached) and re-evaluates the exact action that was evaluated at
preview time: an update payload with more than `BULK_UPDATE_THRESHOLD` (20) patches is
re-checked as `bulk_update`, not plain `update`. Revoking `write` in the desktop app
therefore blocks commits of already-previewed changes.

## Read Gate on Preview Diffs

`preview_update_records` requires `read` in addition to `write`, because the returned
diff contains the current (before) values of the patched records. A write-only rule
cannot be used to exfiltrate table contents through previews.

## Google Sheets URL Parsing (No SSRF)

A Google Sheets `tableId` may be a full spreadsheet URL, a bare spreadsheet id, or
`spreadsheetId:gid` / `spreadsheetId:SheetName` (see `docs/mcp-tools.md`). Parsing this
input is strictly an extraction step and never chooses where a request goes:

- The parser only pulls the spreadsheet id and a tab selector (a numeric `gid` or a sheet
  title) out of the input. It never derives an HTTP host, port, path, or endpoint from it.
  Every Sheets/Drive call is built from the fixed `SHEETS_ENDPOINT`
  (`https://sheets.googleapis.com/v4/spreadsheets`) and the connected account's token, with
  the id and range pushed as percent-encoded path segments via the `url` crate. There is no
  code path where a tableId (or the host in a pasted URL) can redirect a request to another
  origin, so a malicious link cannot be used for SSRF or to reach an internal service.
- URLs are only accepted when the host is a Google document host (`docs.google.com` /
  `drive.google.com`); any other host is rejected rather than followed. The host is used
  solely as a validation gate, not for routing.
- The extracted spreadsheet id must look like a Google document id (URL-safe base64
  alphabet `A-Za-z0-9_-`, and long enough). Ids containing `/`, `:`, spaces, or other URL
  syntax, and short/junk strings, are rejected before any request is built.
- A `gid` or sheet name that does not exist in the spreadsheet surfaces as a `NotFound`
  tool error after a single metadata lookup against the fixed endpoint; it cannot widen the
  request surface.

## Token Handling

- Secrets live in the OS keychain under service `sheet-port`, accessed through the
  `keyring` crate. Google accounts are multi-tenant: each connected account stores its
  tokens under user `google_sheets:{accountKey}` (accountKey = the sanitized email), and
  the shared, single-OAuth-app client secret lives under user `google_client_secret`.
  The `provider` connector reserves user `provider`.
- Raw tokens never leave the `google` module. Connectors obtain a short-lived access
  token per account via the crate-private `google::access_token(conn, sourceId)`, which
  refreshes through that account's own refresh token when expired.
- The `token_status` Tauri command returns only booleans; `googleSheets` reflects
  whether any account is connected (derived from the keyed source rows, since the
  keychain cannot be enumerated). Secrets never cross Tauri IPC and are never exposed to
  agents or the frontend.
- Keychain errors are logged to stderr and reported as "absent" rather than leaking
  error details to the UI.
- Backward compatibility: a pre-multi-account single connection is migrated into the
  keyed scheme once on startup (idempotent, best-effort); the legacy `google_sheets`
  keychain entry is cleared after the tokens move to their keyed entry.

If agents received provider credentials they could bypass policy, audit, preview, and
confirmation entirely. Keeping tokens behind the local broker means every action can be
checked, logged, and shaped through narrow schemas.

## Desktop Hardening (CSP + Capabilities)

- CSP (`tauri.conf.json`): `default-src 'self'; script-src 'self'; style-src 'self'
  'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self' ipc:
  http://ipc.localhost`. No remote origins, no `unsafe-eval`, no external script
  loading.
- Capabilities (`src-tauri/capabilities/default.json`): the main window gets only
  `core:default` plus the four window permissions needed by the custom titlebar
  (minimize, toggle-maximize, close, start-dragging). No fs, shell, http, or dialog
  permissions are granted.
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
- stdio remains the default and needs no port. Switching transports requires a sidecar
  restart to take effect.
- For the HTTP transport the desktop app can manage the sidecar as a child process
  (`mcp_server_start` / `mcp_server_stop`, see `docs/ipc.md`): it spawns the resolved
  `sheet-port-mcp` binary with `SHEET_PORT_MCP_TRANSPORT=http` and the configured port,
  tracks a single child, and kills it on app exit so no orphan server lingers. stdio
  clients spawn their own sidecar and are never managed this way.

The same permission checks, preview -> approve -> commit enforcement, audit logging, and
heartbeat apply identically on both transports - only the wire transport differs, not the
broker guarantees.

Tool input schemas are strict, bounded, and provider-neutral, enforced in
`crates/sheet-port-mcp/src/args.rs`: page limits 1-500, query max 200 chars, at most 100
patches/records per change. Out-of-range input surfaces as a clear tool error, never as a
raw schema failure.

## Tool Allowlist

Allowed tool categories:

- list data sources, list tables, describe schema
- bounded reads and text search
- preview updates and appends as pending changes
- commit a pending change (after the enforcement above)
- read the audit log

Not exposed, by design:

- shell command execution
- arbitrary JavaScript execution
- raw provider API calls
- token export
- SQL execution
- destructive deletes (no delete tool exists in the MVP)

## Permission Checks

Read tools require `read`. Write previews and commits require `write`. Updates touching
more than 20 records are evaluated as `bulk_update`, which can carry its own
confirmation requirement. Delete operations would require `deleteRecords` but are not
implemented. Rule precedence: an exact `(sourceId, tableId)` rule wins over a
source-wide rule.

## Audit Log

Audit events persist in the shared SQLite `audit_events` table with timestamp, actor
(`user` / `agent` / `system`), action, source/table scope, and JSON metadata. Both
processes write to it: the sidecar records every tool call (including previews and
commits), the desktop records permission edits and approve/reject decisions. Events
survive restarts of either process.

## Current Limitations

- No delete flow: delete changes are typed but rejected by the commit path.
- The provider connector is still a stub; the Google Sheets and mock connectors are
  functional.
- The SQLite database is unencrypted at rest; any local process running as the same OS
  user can read or modify it (including permission rules and pending changes).
- For the stdio transport the desktop app does not manage the sidecar lifecycle; the
  agent's MCP client spawns it. Only the optional HTTP sidecar can be desktop-managed.
