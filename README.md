# Airtable - Sheet Port

Airtable - Sheet Port is a safe local port for AI agents to access tables and
spreadsheets.

It is a desktop permission broker for Google Sheets and local table workflows. The app
owns tokens and local policy, while AI agents interact only through a narrow local MCP
server with typed tools for reading, previewing, and committing table changes.

## Current Status

Working end to end today (against the built-in mock connector):

- Two local processes share one SQLite database (WAL): the Tauri desktop app
  (Rust backend, rusqlite) and the Node MCP sidecar (`node:sqlite`). No direct IPC;
  each reads fresh state from the DB.
- MCP sidecar with 9 typed tools (`list_sources`, `list_tables`, `describe_table`,
  `read_table`, `find_records`, `preview_update_records`, `append_records`,
  `commit_change`, `get_audit_log`) with strict zod bounds.
- Enforced approval flow: writes become pending changes with diffs;
  `commit_change` refuses changes that require confirmation until the user approves
  them in the desktop app; status transitions are atomic guarded UPDATEs; permissions
  are re-checked at commit time.
- Desktop UI live-wired via typed Tauri IPC (`docs/ipc.md`): Dashboard (sidecar
  heartbeat status), Data Sources, Tables, Permissions editor, Changes
  (approve/reject with diff viewer), Audit Log. Custom titlebar
  (`decorations: false`), tight CSP, minimal capabilities.
- Persistent audit log written by both processes (agent tool calls, user decisions,
  permission edits).
- SQLite-backed mock connector shared by the desktop UI and the sidecar; committed
  changes persist and show up in both.
- Keyring stub (service `sheet-port`): the desktop reports whether token entries exist;
  secrets never cross IPC.

Not yet: real Google OAuth, functional Google Sheets / provider connectors, delete
flows, DB encryption at rest.

## Tech Stack

- Monorepo: pnpm workspaces, strict TypeScript project references
- Desktop: Tauri 2 (Rust backend with rusqlite; React 18 + Vite frontend)
- UI: Tailwind CSS, TanStack Query + Table, lucide-react
- MCP: `@modelcontextprotocol/sdk` (stdio transport), zod validation
- Persistence: shared SQLite (WAL); schema/seed in `packages/storage`, embedded by both
  Rust (`include_str!`) and Node
- Secrets: OS keychain via the `keyring` crate (stub for now)

## Repo Structure

```txt
apps/
  desktop/            React/Vite frontend + Tauri 2 Rust backend
    src/              Screens, hooks, typed IPC client (browser demo fallback)
    src-tauri/        db.rs, queries.rs, commands.rs, models.rs
  mcp-server/         Node MCP sidecar (stdio), 9 typed tools, heartbeat
packages/
  shared/             Shared types + BULK_UPDATE_THRESHOLD
  core/               Domain services + storage ports (permissions, changes, audit,
                      connector registry, schema)
  storage/            SQLite layer (node:sqlite) + schema.sql + seed.sql shared with Rust
  ui/                 Small UI helpers
  connectors/
    mock/             SQLite-backed mock connector (shared with the desktop UI)
    google-sheets/    Skeleton (OAuth + range mapping TODO)
    provider/         Skeleton (additional provider TODO)
docs/                 Scope, architecture, security, MCP tools, connectors, development,
                      IPC contract (docs/ipc.md is canonical)
examples/             Claude Desktop config
```

## Quick Start

```bash
pnpm install
pnpm build
```

Run the desktop app (requires the Rust toolchain and Tauri 2 prerequisites):

```bash
pnpm --filter @sheet-port/desktop tauri:dev
```

Run tests:

```bash
pnpm test
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
```

Connect Claude Desktop: copy `examples/claude-desktop-config.json` into your Claude
Desktop configuration and adjust the absolute path. The config runs the sidecar's
`start` script (`node dist/index.js`), so `pnpm build` must have been run first.

Both processes share the same database
(Windows `%APPDATA%\sheet-port\sheet-port.db`; see `docs/development.md` for macOS and
Linux paths and the `SHEET_PORT_DB` override).

## Dev Scripts

| Command | What it does |
|---|---|
| `pnpm dev` | All packages in watch mode (parallel) |
| `pnpm build` | Build all TS packages and apps |
| `pnpm typecheck` | Strict TypeScript, no emit |
| `pnpm test` | TS unit + integration tests (incl. MCP e2e smoke) |
| `pnpm lint` / `pnpm format` | Lint / Prettier |
| `pnpm --filter @sheet-port/desktop dev` | Frontend only at `http://127.0.0.1:8477` (demo fixtures) |
| `pnpm --filter @sheet-port/desktop tauri:dev` | Full desktop app (Rust + React) |
| `pnpm --filter @sheet-port/mcp-server dev` | Sidecar via tsx (stdio) |
| `pnpm --filter @sheet-port/mcp-server start` | Sidecar from `dist/` (build first) |
| `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` | Rust backend tests |

## Security Note

Agents never receive provider OAuth tokens, API keys, raw provider API access, shell
execution, JavaScript execution, or unrestricted writes. Every write is a pending change
with a diff; changes flagged by policy require user approval in the desktop app before
`commit_change` succeeds, permissions are re-checked at commit, and everything is
audited to SQLite. See `docs/security.md`.

## Roadmap

- Real Google OAuth in Tauri with OS keychain token storage
- Functional Google Sheets connector (range-to-record mapping)
- Additional provider connector (bases, field type mapping, rate limits)
- Delete flow with explicit confirmation semantics
- Database encryption at rest
- UI polish: approval notifications, policy presets, richer diff views
