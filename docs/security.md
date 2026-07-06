# Security

## Threat Model

Airtable - Sheet Port assumes AI agents may be over-permissioned, prompt-injected, or
confused by untrusted spreadsheet content. The app reduces blast radius by exposing
typed local tools instead of provider credentials or broad execution primitives, and by
forcing every write through a persisted preview -> approve -> commit pipeline.

## Confirmation Enforcement (Cross-Process)

Confirmation is enforced, not advisory. The desktop app and the MCP sidecar share one
SQLite database (see `docs/ipc.md`, the canonical contract), and the sidecar reads fresh
state on every call, so a desktop decision applies immediately without any direct IPC.

Mechanism:

1. `preview_update_records` / `append_records` insert a `pending_changes` row with
   `requires_confirmation` snapshotted from the matching permission rule
   (`requireConfirmationFor` containing the evaluated action). The flag is returned to
   the agent as `requiresConfirmation` so it knows to ask the user.
2. `commit_change` re-reads the row and refuses to proceed when:
   - the change is unknown,
   - status is `rejected` (the user declined in the desktop app),
   - status is `committed` (already applied),
   - `requires_confirmation` is set and status is not `approved` (the agent is told to
     ask the user to approve in the desktop app).
3. Only when `requires_confirmation` is false may a `pending` change auto-approve with
   `decided_by = 'policy'`.

All status transitions are atomic guarded UPDATEs, so concurrent actors cannot race a
change into an invalid state or apply it twice:

- Desktop approve/reject (Rust): `UPDATE pending_changes SET status = ?, decided_at = ?,
  decided_by = 'user' WHERE id = ? AND status = 'pending'`; zero affected rows is
  reported as an error with the actual current status.
- Sidecar policy approval (Node): the same guarded `pending -> approved` transition with
  `decided_by = 'policy'`.
- Commit finalization: `UPDATE ... SET status = 'committed' WHERE id = ? AND
  status = 'approved'`; a missed guard aborts the commit with an error.

## Permission Re-Check at Commit

Permission rules may change between preview and commit. `ChangeService.commit` re-reads
the rules (they are never cached) and re-evaluates the exact action that was evaluated
at preview time: an update payload with more than `BULK_UPDATE_THRESHOLD` (20) patches
is re-checked as `bulk_update`, not plain `update`. Revoking `write` in the desktop app
therefore blocks commits of already-previewed changes.

## Read Gate on Preview Diffs

`preview_update_records` requires `read` in addition to `write`, because the returned
diff contains the current (before) values of the patched records. A write-only rule
cannot be used to exfiltrate table contents through previews.

## Token Handling

- Secrets live in the OS keychain under service `sheet-port` (users `google_sheets` and
  `provider`). The current implementation is a stub: no flow writes tokens yet.
- The `token_status` Tauri command returns only booleans (entry exists / does not
  exist). Secrets never cross Tauri IPC and are never exposed to agents or the frontend.
- Keychain errors are logged to stderr and reported as "absent" rather than leaking
  error details to the UI.

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

The MCP server runs locally with stdio transport. If HTTP/SSE is ever added it must
bind only to `127.0.0.1`. Tool input schemas are strict, bounded, and provider-neutral
(zod: limits 1-500, query max 200 chars, at most 100 patches/records per change).

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

- No real OAuth yet: the keyring integration is a stub and no connector consumes tokens.
- No delete flow: delete changes are typed but rejected by `ChangeService`.
- Only the mock connector is functional; Google Sheets and the additional provider are
  skeletons.
- The SQLite database is unencrypted at rest; any local process running as the same OS
  user can read or modify it (including permission rules and pending changes).
