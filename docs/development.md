# Development

## Prerequisites

- Node.js 20 or newer
- pnpm 9 or newer
- Rust toolchain for full Tauri desktop builds
- Platform prerequisites for Tauri 2

## Install

```bash
pnpm install
```

## Dev Commands

```bash
pnpm dev
pnpm --filter @sheet-port/mcp-server dev
pnpm --filter @sheet-port/desktop dev
```

## Build Commands

```bash
pnpm build
pnpm typecheck
```

## How to Run Desktop

For frontend-only development:

```bash
pnpm --filter @sheet-port/desktop dev
```

For the Tauri shell after installing Rust/Tauri prerequisites:

```bash
pnpm --filter @sheet-port/desktop tauri:dev
```

## How to Run MCP Server

```bash
pnpm --filter @sheet-port/mcp-server dev
```

The server uses stdio transport and mock data by default.

## How to Add a New MCP Tool

1. Add the input schema in `apps/mcp-server/src/tools.ts`.
2. Route through core services or connector registry.
3. Add permission checks before reading or writing.
4. Record an audit event.
5. Document the tool in `docs/mcp-tools.md`.

## How to Add a New Connector

1. Create a package under `packages/connectors/<provider>`.
2. Implement the `TableConnector` interface from `@sheet-port/shared`.
3. Keep credentials inside app-owned auth services.
4. Register the connector in the app or MCP bootstrap.
5. Add provider mapping notes to `docs/connectors.md`.

## Current Limitations

- No SQLite migrations yet.
- No secure storage implementation yet.
- Desktop and MCP share design contracts but not a persistent runtime store yet.
