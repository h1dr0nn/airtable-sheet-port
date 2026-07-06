# Sheet Port

Safe local port for AI agents to access tables and spreadsheets.

Sheet Port is a desktop permission broker for Google Sheets now and Airtable later. The app owns OAuth tokens and local policy, while AI agents interact only through a narrow local MCP server with typed tools for reading, previewing, and committing table changes.

## Current Status

This repository contains the first runnable base:

- pnpm monorepo scaffold
- React/Vite/Tauri desktop app scaffold
- shared TypeScript types
- core permission, audit, schema, change, and connector registry services
- mock connector with example source, table, schema, records, append, and update
- local MCP server using mock data
- Google Sheets and Airtable connector skeletons with explicit auth TODOs
- docs for product scope, architecture, security, MCP tools, connectors, and development

## Tech Stack

- Monorepo: pnpm
- Desktop: Tauri 2 scaffold
- Frontend: React, Vite, TypeScript
- UI: Tailwind CSS with local shadcn-style primitives
- Table UI: TanStack Table
- MCP: `@modelcontextprotocol/sdk`
- Validation: zod
- Connectors: mock now, Google Sheets and Airtable skeletons
- Local persistence target: SQLite
- Secure storage target: OS keychain abstraction

## Repo Structure

```txt
apps/
  desktop/       React/Vite/Tauri desktop app
  mcp-server/    Local MCP server sidecar
packages/
  shared/        Shared table, permission, change, and audit types
  core/          Registry and domain services
  ui/            Small local UI primitives
  connectors/
    google-sheets/
    airtable/
docs/
examples/
```

## Quick Start

```bash
pnpm install
pnpm dev
```

Run individual apps:

```bash
pnpm --filter @sheet-port/mcp-server dev
pnpm --filter @sheet-port/desktop dev
```

## Dev Scripts

```bash
pnpm build
pnpm typecheck
pnpm lint
pnpm format
```

## Security Note

Agents must never receive provider OAuth tokens, API keys, raw Google API access, shell execution, JavaScript execution, or unrestricted write access. All writes are represented as pending changes and must pass permission checks before commit.

## Roadmap

- Persist audit logs and pending changes in SQLite
- Add OS keychain token storage
- Complete Google OAuth and Sheets range mapping
- Add Airtable auth and base/table discovery
- Add desktop approval workflow connected to the MCP sidecar
- Add signed local sidecar lifecycle management from Tauri
