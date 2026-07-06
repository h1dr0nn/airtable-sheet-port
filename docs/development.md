# Development

## Prerequisites

- Node.js 24 or newer (the storage layer uses the built-in `node:sqlite` module)
- pnpm 9 (the repo pins `packageManager: pnpm@9.x`)
- Rust toolchain (stable) plus the Tauri 2 platform prerequisites for desktop builds

## Install

```bash
pnpm install
```

## Dev Commands

```bash
pnpm dev                                   # all packages in watch mode (parallel)
pnpm --filter @sheet-port/mcp-server dev   # sidecar via tsx (stdio transport)
pnpm --filter @sheet-port/desktop dev      # frontend only, Vite dev server
pnpm --filter @sheet-port/desktop tauri:dev # full desktop app (Rust + React)
```

The frontend intentionally uses the app-specific local port `8477`
(`http://127.0.0.1:8477`). In a plain browser (without Tauri) the UI falls back to
in-memory demo fixtures; run `tauri:dev` to exercise the real Rust backend, the custom
titlebar (the window uses `decorations: false`), and the shared database.

## Build Commands

```bash
pnpm build       # all TS packages + apps
pnpm typecheck   # tsc project references, no emit
pnpm lint
pnpm format
```

## Test Commands

```bash
pnpm test                                                     # TS unit + integration tests
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml  # Rust backend tests
```

The MCP end-to-end smoke runs as part of the TS test suite: it builds/starts the sidecar
over stdio against a temp database (via `SHEET_PORT_DB`) and drives the
preview -> approve -> commit flow. Rust tests use isolated temp-file databases and never
touch your real data.

## Shared Database

Both processes open the same SQLite file (WAL mode). Locations:

| OS | Path |
|---|---|
| Windows | `%APPDATA%\sheet-port\sheet-port.db` |
| macOS | `~/Library/Application Support/sheet-port/sheet-port.db` |
| Linux | `$XDG_DATA_HOME/sheet-port/sheet-port.db`, else `~/.local/share/sheet-port/sheet-port.db` |

Override with the `SHEET_PORT_DB` environment variable (absolute file path). This is how
tests and smoke scripts isolate state. Whichever process opens the DB first applies
`packages/storage/schema.sql` and `packages/storage/seed.sql` (both idempotent). To
reset local state, stop both processes and delete the DB file (plus its `-wal`/`-shm`
siblings).

`docs/ipc.md` is the canonical contract for the Tauri commands and the shared-state
model; keep `apps/desktop/src/lib/ipc.ts` and `src-tauri/src/models.rs` in sync with it.

## How to Run the MCP Server

```bash
pnpm --filter @sheet-port/mcp-server dev     # tsx, for development
pnpm build && pnpm --filter @sheet-port/mcp-server start   # node dist/index.js
```

The server uses stdio transport. For Claude Desktop use
`examples/claude-desktop-config.json`; note that the `start` script runs
`node dist/index.js`, so `pnpm build` must run first.

## How to Add a New MCP Tool

1. Register it in `apps/mcp-server/src/tools.ts` via `server.registerTool` with a strict,
   bounded zod input schema (follow the existing limits: list sizes 1-100, page limits
   1-500, query strings capped). Add `annotations: READ_ONLY` for read-only tools.
2. Check permissions first through `context.permissions`
   (`assertCanRead` / `assertCanWrite`). Writes must go through `context.changes`
   (preview + commit), never directly through a connector.
3. Route data access through `context.registry` (never a concrete connector).
4. Record an audit event through `context.audit` with actor `agent` and useful metadata.
5. Document the tool in `docs/mcp-tools.md` (input bounds, output shape, permission,
   example call and response).

## How to Add a New Connector

1. Create a package under `packages/connectors/<provider>` implementing the
   `TableConnector` interface from `@sheet-port/shared` with a unique `kind`.
2. If the kind is new, add it to `DataSourceKind` in `packages/shared` and to the
   `sources.kind` CHECK constraint in `packages/storage/schema.sql` (mirror the change
   in `packages/storage/src/sql.ts`).
3. Register it in `createAppContext` (`apps/mcp-server/src/context.ts`) via
   `registry.register(...)`. The registry routes by the `sources.kind` column, so a
   source row with your kind is all the wiring the router needs.
4. Keep credentials inside desktop-owned services (OS keychain, service `sheet-port`).
   Connectors must never receive tokens through MCP tool inputs.
5. Add provider mapping notes to `docs/connectors.md`.

## Current Limitations

- No SQLite schema migration mechanism yet (only the idempotent initial schema plus a
  `schema_version` meta key).
- The keyring integration is a stub: `token_status` reads entries but no flow writes
  them.
- The desktop app does not manage the sidecar lifecycle; MCP clients spawn the server.
