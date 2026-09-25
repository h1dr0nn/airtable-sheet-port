# Product Scope

## Problem

AI agents are useful for spreadsheet cleanup, enrichment, reporting, and operational
data maintenance, but handing them Google credentials or raw API access is risky and
hard to audit. A local broker should let agents read and change spreadsheets through
narrow, typed tools, with every write recorded as a diff, without the agent ever holding
a token.

## Target Users

- Operators who manage Google Sheets as lightweight databases.
- Developers building local AI workflows around tabular data.
- Teams that want an audit trail of what agents changed and per-source read / write /
  delete limits.
- Users who cannot or do not want to create a Google Cloud project and OAuth client just
  to let an agent use their sheets.

## Main Use Cases

- Connect one or more Google accounts by deploying a small Apps Script bridge on each.
- Let an agent list accounts, spreadsheets and tabs, inspect schema, and read bounded
  rows, formulas, raw cells, and styles.
- Let an agent paste a spreadsheet link and have the broker pick the account that can
  open it.
- Let an agent update records, write cells, append rows, format tabs, and create or
  delete tabs, receiving the diff of what it changed.
- Let an agent stage a change with `dryRun` and commit it later.
- Keep a persistent audit trail of agent reads and writes, visible in the desktop audit log.

## Non-Goals

- Airtable - Sheet Port is not a full spreadsheet editor.
- Airtable - Sheet Port is not a cloud-hosted proxy; the broker runs locally and the
  only remote piece is the user's own Apps Script bridge.
- It does not expose raw Google APIs, tokens, or bridge secrets to agents.
- It does not provide a human approval gate. Approving agent actions belongs to the
  agent harness (see `docs/security.md`).
- It does not sync every spreadsheet feature: charts, conditional formatting, pivot
  tables and similar are out of scope.

## Current Status (2.0.0)

- Desktop app: Dashboard, Data Sources (Google bridges), Tables (workbench), and Settings (Google bridges, permissions, MCP server and clients, appearance, updates),
  live-wired to the Rust backend via typed Tauri IPC (`docs/ipc.md`).
- Local Rust MCP sidecar (`crates/sheet-port-mcp`) with 18 tools over stdio or loopback
  HTTP. `sourceId` is optional and routed automatically.
- Google access through Apps Script bridges: a pool of accounts, one source per account,
  credentials and cached tokens in the OS keychain. No OAuth client or Cloud Console.
- Writes commit in one call by default through the staged-change pipeline (diff, audit,
  permission re-check); `dryRun` stages only.
- Permission rules: read, write, delete per source or spreadsheet. New bridge sources
  allow all three.
- The sidecar works without the desktop app running.
- Signed releases with auto-update for Windows, Linux and macOS.

Removed in 2.0.0: the OAuth flow and client id/secret settings, the desktop approve step
and auto-approve setting, per-action confirmation lists and `requiresConfirmation`, the
provider stub connector, the mock connector in release builds, and the browser demo
fixtures outside Vite dev mode.

## Future Roadmap

- Stable record ids (hidden id column) so row inserts and sorts do not shift targets.
- Rate-limit and retry handling for Google API errors.
- Database encryption at rest.
- Optional policy presets (read-only, no-delete) applied when a bridge is added.
- Additional connectors through the `TableConnector` trait.

## Assumptions

- The default MCP transport is stdio for compatibility with Claude Desktop and other
  local agent clients; the MCP client spawns the sidecar.
- The agent harness is where a person approves or denies tool calls.
- Users keep bridge secrets private; a leaked URL + secret grants token access to that
  account outside this app.

## Current Limitations

- Google Sheets is the only production connector.
- No record-level delete; only whole tabs can be deleted.
- Record ids are sheet row numbers and shift with structural edits.
- The shared SQLite database is unencrypted at rest.
- For stdio, the desktop app does not manage the sidecar lifecycle.
