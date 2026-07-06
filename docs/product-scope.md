# Product Scope

## Problem

AI agents are useful for spreadsheet cleanup, enrichment, reporting, and operational
data maintenance, but direct access to Google Sheets or other provider credentials is
too risky. A local broker should let agents inspect data and propose changes without
receiving provider tokens or bypassing user policy.

## Target Users

- Operators who manage Google Sheets or local tables as lightweight databases.
- Developers building local AI workflows around tabular data.
- Teams that want auditability and approval gates before agents modify business data.
- Power users who want a safer alternative to sharing OAuth tokens or API keys with
  agents.

## Main Use Cases

- Let an agent list connected sources and tables.
- Let an agent inspect table schema and read bounded rows.
- Let an agent search records by text.
- Let an agent preview record updates or appends and receive a diff.
- Require user approval in the desktop app before risky commits (enforced, not
  advisory).
- Keep a persistent audit trail of agent reads, previews, decisions, and writes.

## Non-Goals

- Airtable - Sheet Port is not a full spreadsheet editor.
- Airtable - Sheet Port is not a cloud-hosted proxy.
- Airtable - Sheet Port does not expose raw provider APIs to agents.
- The MVP does not implement destructive delete flows.
- The MVP does not sync every spreadsheet feature such as formulas, charts, formatting,
  or pivot tables.

## MVP Status

The MVP is runnable end to end against the mock connector, with the whole broker
implemented in Rust (a shared core crate wrapped by the Tauri desktop app and the MCP
sidecar):

- Desktop app with Dashboard, Data Sources, Tables, Permissions, Changes, Audit Log,
  and Settings (light/dark/system theme) screens, live-wired to the Rust backend via
  typed Tauri IPC (`docs/ipc.md`).
- Local Rust MCP sidecar (`crates/sheet-port-mcp`) with 9 strict table-oriented tools
  (stdio transport).
- Shared SQLite database (WAL) used by both processes: sources, permission rules,
  pending changes, audit events, mock data, and the sidecar heartbeat.
- Enforced approval flow: previews create pending changes; `commit_change` refuses
  changes that require confirmation until the user approves them in the desktop app;
  permissions are re-checked at commit time.
- SQLite-backed mock connector shared by the desktop UI and the sidecar; committed
  changes persist and appear in both.
- Google Sheets and additional provider connectors exist as Rust stubs with explicit
  integration TODOs; a keyring stub reports token presence.

## Future Roadmap

- Google OAuth flow in Tauri with real OS keychain token storage.
- Real Google Sheets connector with range-to-record mapping.
- Additional table-provider connectors with source, table, field, and record mapping.
- Delete flow with explicit confirmation semantics.
- Database encryption at rest.
- Desktop approval notifications for pending changes created by MCP calls.
- Policy presets for read-only, safe update, and bulk-change lockdown modes.
- Connector SDK for additional providers.

## Assumptions

- The first MCP transport is stdio for compatibility with Claude Desktop and local agent
  clients.
- The MCP client (not the desktop app) spawns the sidecar; the desktop observes it
  through the heartbeat table.
- Real token storage is intentionally deferred behind a connector auth abstraction plus
  the existing keyring stub.
- Delete operations remain out of the runnable MVP until confirmation and policy
  semantics are stronger.

## Current Limitations

- Only the mock connector is functional; the Google Sheets and additional provider
  connectors are stubs that return "not implemented" errors.
- No real OAuth yet; the keyring integration is a stub that no flow writes to.
- No delete flow.
- The shared SQLite database is unencrypted at rest.
- The desktop app does not manage the sidecar lifecycle; MCP clients spawn it.
