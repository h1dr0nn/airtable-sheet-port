# Architecture

## High-Level Architecture

Sheet Port is a local desktop app plus a local MCP sidecar. The desktop app owns identity, policy, approval, and audit UX. The MCP sidecar exposes a small set of table tools to agents and delegates all domain decisions to shared core services.

```mermaid
flowchart LR
  Agent["AI Agent"] --> MCP["Local MCP Server"]
  MCP --> Core["Core Services"]
  Core --> Registry["Connector Registry"]
  Registry --> Mock["Mock Connector"]
  Registry --> Sheets["Google Sheets Connector"]
  Registry --> Airtable["Airtable Connector"]
  Desktop["Tauri Desktop App"] --> Core
  Core --> Store["SQLite + Secure Storage (planned)"]
```

## Desktop App

The desktop app is a React/Vite/Tauri shell. The initial UI shows:

- Dashboard with MCP and connector state.
- Data Sources with Google Sheets and Airtable placeholders.
- Tables with mock table preview.
- Permissions editor for base rule shape.
- Changes screen for pending diffs and mock approvals.
- Audit Log for tool calls and write actions.

Future Tauri commands will start, stop, and monitor the MCP sidecar and expose local persisted state to the UI.

## MCP Server

The MCP server uses stdio transport by default. It exposes only allowlisted tools with zod schemas:

- `list_sources`
- `list_tables`
- `describe_table`
- `read_table`
- `find_records`
- `preview_update_records`
- `append_records`
- `commit_change`
- `get_audit_log`

It does not expose shell execution, JavaScript execution, provider tokens, or raw provider API calls.

## Core Package

The core package contains domain services:

- `ConnectorRegistry`: routes calls to connectors by source kind and source id.
- `PermissionService`: evaluates read/write/delete and confirmation requirements.
- `AuditService`: records security-relevant events.
- `ChangeService`: creates and commits pending changes.
- `SchemaService`: caches and validates table schemas.

## Connector Layer

Connectors implement a table abstraction rather than provider-specific APIs. The mock connector is complete enough for development. Google Sheets and Airtable packages currently define the boundary and TODOs for auth and mapping.

## Data Flow

```mermaid
sequenceDiagram
  participant A as Agent
  participant M as MCP Server
  participant P as PermissionService
  participant C as ConnectorRegistry
  participant X as Connector
  participant L as AuditService

  A->>M: read_table(sourceId, tableId)
  M->>P: assertCanRead
  P-->>M: allowed
  M->>C: readTable
  C->>X: readTable
  X-->>C: records
  C-->>M: records
  M->>L: record read event
  M-->>A: records
```

## Preview to Commit Flow

```mermaid
sequenceDiagram
  participant A as Agent
  participant M as MCP Server
  participant P as PermissionService
  participant Ch as ChangeService
  participant C as ConnectorRegistry
  participant L as AuditService

  A->>M: preview_update_records(patches)
  M->>P: assertCanWrite(update)
  P-->>M: allowed, confirmation required
  M->>Ch: createUpdateChange
  Ch-->>M: pending change with diff
  M->>L: record preview event
  M-->>A: changeId and diff
  A->>M: commit_change(changeId)
  M->>P: assertCanWrite(update)
  M->>Ch: commitChange
  Ch->>C: updateRecords
  C-->>Ch: updated records
  Ch-->>M: committed change
  M->>L: record commit event
  M-->>A: committed result
```

## Current Limitations

- Persistence is in-memory.
- Desktop approval and MCP pending-change state are not yet connected across processes.
- Tauri sidecar packaging is scaffolded but not complete.
