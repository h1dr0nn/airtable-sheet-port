# Product Scope

## Problem

AI agents are useful for spreadsheet cleanup, enrichment, reporting, and operational data maintenance, but direct access to Google Sheets or Airtable credentials is too risky. A local broker should let agents inspect data and propose changes without receiving provider tokens or bypassing user policy.

## Target Users

- Operators who manage Google Sheets or Airtable as lightweight databases.
- Developers building local AI workflows around tabular data.
- Teams that want auditability and approval gates before agents modify business data.
- Power users who want a safer alternative to sharing OAuth tokens or API keys with agents.

## Main Use Cases

- Let an agent list connected sources and tables.
- Let an agent inspect table schema and read bounded rows.
- Let an agent search records by text.
- Let an agent preview record updates and receive a diff.
- Let a user or policy layer approve a pending change before commit.
- Keep an audit trail of agent reads, previews, and writes.

## Non-Goals

- Airtable - Sheet Port is not a full spreadsheet editor.
- Airtable - Sheet Port is not a cloud-hosted proxy.
- Airtable - Sheet Port does not expose raw provider APIs to agents.
- The MVP does not implement destructive delete flows.
- The MVP does not sync every spreadsheet feature such as formulas, charts, formatting, or pivot tables.

## MVP

- Desktop shell with Dashboard, Data Sources, Tables, Permissions, Changes, and Audit Log screens.
- Local MCP server with strict table-oriented tools.
- Mock connector for development and tests.
- Shared type layer and core services for permission, audit, schema, change, and connector routing.
- Google Sheets and Airtable connector packages with clear integration TODOs.

## Future Roadmap

- Google OAuth flow in Tauri with OS keychain token storage.
- SQLite persistence for sources, schemas, permissions, pending changes, and audit events.
- Real Google Sheets connector with range-to-record mapping.
- Airtable connector with base, table, field, and record mapping.
- Desktop approval notifications for pending changes created by MCP calls.
- Policy presets for read-only, safe update, and bulk-change lockdown modes.
- Connector SDK for additional providers.

## Assumptions

- The first MCP transport is stdio for compatibility with Claude Desktop and local agent clients.
- The app will later own sidecar lifecycle, but the server can be run independently during development.
- Real token storage is intentionally deferred behind a connector auth abstraction.
- Delete operations remain out of the runnable MVP until confirmation and policy semantics are stronger.

## Current Limitations

- Data is in-memory only.
- Desktop UI uses mock state and is not yet wired to the MCP sidecar.
- Google Sheets and Airtable packages are skeletons.
- Tauri Rust commands are minimal placeholders.
