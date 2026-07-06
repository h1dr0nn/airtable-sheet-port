# Airtable - Sheet Port

Airtable - Sheet Port is a safe local port for AI agents to access tables and
spreadsheets.

It is a desktop permission broker for Google Sheets and local table workflows. The app
owns tokens and local policy, while AI agents interact only through a narrow local MCP
server with typed tools for reading, previewing, and committing table changes.

## Current Status

Working end to end today (against the built-in mock connector):

- The entire broker is Rust. Two local processes share one SQLite database (WAL):
  the Tauri desktop app and the Rust MCP sidecar (`crates/sheet-port-mcp`). Both are
  thin shells over one core crate (`crates/sheet-port-core`) that owns all broker
  logic. No direct IPC; each process reads fresh state from the DB.
- MCP sidecar with 9 typed tools (`list_sources`, `list_tables`, `describe_table`,
  `read_table`, `find_records`, `preview_update_records`, `append_records`,
  `commit_change`, `get_audit_log`) with strict input bounds.
- Enforced approval flow: writes become pending changes with diffs;
  `commit_change` refuses changes that require confirmation until the user approves
  them in the desktop app; status transitions are atomic guarded UPDATEs; permissions
  are re-checked at commit time.
- Desktop UI live-wired via typed Tauri IPC (`docs/ipc.md`): Dashboard (sidecar
  heartbeat status), Data Sources, Tables, Permissions editor, Changes
  (approve/reject with diff viewer), Audit Log, and Settings with a dual light/dark
  theme (Light / Dark / System, persisted in localStorage). Custom titlebar
  (`decorations: false`), tight CSP, minimal capabilities.
- Persistent audit log written by both processes (agent tool calls, user decisions,
  permission edits).
- SQLite-backed mock connector shared by the desktop UI and the sidecar; committed
  changes persist and show up in both.
- Keyring stub (service `sheet-port`): the desktop reports whether token entries exist;
  secrets never leave the Rust process or the OS keychain.

Not yet: real Google OAuth, functional Google Sheets / provider connectors, delete
flows, DB encryption at rest.

## Tech Stack

- Broker: Rust workspace (`Cargo.toml` at the repo root)
  - `crates/sheet-port-core`: permissions, change lifecycle, audit, connectors,
    heartbeat, keychain vault, shared SQLite access (rusqlite with bundled SQLite)
  - `crates/sheet-port-mcp`: stdio MCP sidecar built on `rmcp` (tool schemas via
    `schemars`, async runtime `tokio`)
  - `apps/desktop/src-tauri`: Tauri 2 shell; thin `#[tauri::command]` wrappers over
    the core crate
- Frontend: React 18 + Vite, Tailwind CSS, TanStack Query + Table, Radix primitives
  (`packages/ui`), lucide-react; TypeScript types mirrored in `packages/shared`
- Persistence: shared SQLite (WAL); schema/seed live once at
  `crates/sheet-port-core/sql/` and are embedded via `include_str!`
- Secrets: OS keychain via the `keyring` crate (stub for now)
- Monorepo glue: pnpm workspaces for the frontend packages only

## Repo Structure

```txt
crates/
  sheet-port-core/    Broker core: db.rs, permissions.rs, changes.rs, audit.rs,
                      heartbeat.rs, mock_data.rs, sources.rs, vault.rs, connectors/
    sql/              schema.sql + seed.sql (single source of truth)
  sheet-port-mcp/     Rust MCP sidecar (stdio), the 9 typed tools, heartbeat task
apps/
  desktop/            React/Vite frontend + Tauri 2 Rust shell
    src/              Screens (incl. Settings/theme), hooks, typed IPC client
                      (browser demo fallback)
    src-tauri/        commands.rs (thin wrappers over sheet-port-core)
packages/
  shared/             TypeScript types for the frontend (mirrors docs/ipc.md)
  ui/                 Small React UI primitives (Radix-based)
docs/                 Scope, architecture, security, MCP tools, connectors,
                      development, IPC contract (docs/ipc.md is canonical)
examples/             Claude Desktop config
scripts/              e2e-smoke.mjs (protocol-level MCP smoke test)
```

## Quick Start

Build the MCP sidecar (requires the Rust toolchain):

```bash
cargo build --release -p sheet-port-mcp
```

Run the desktop app (requires Node 20+, pnpm 9, and the Tauri 2 prerequisites):

```bash
pnpm install
pnpm --filter @sheet-port/desktop tauri:dev
```

Run tests:

```bash
cargo test --workspace          # all broker logic (core + MCP crates)
cargo build -p sheet-port-mcp   # debug binary needed by the e2e smoke
pnpm test                       # frontend vitest + MCP e2e smoke
```

Connect Claude Desktop: copy `examples/claude-desktop-config.json` into your Claude
Desktop configuration and adjust the absolute path. The config launches the release
binary at `target/release/sheet-port-mcp.exe` (`sheet-port-mcp` on macOS/Linux), so
run `cargo build --release -p sheet-port-mcp` first.

Both processes share the same database
(Windows `%APPDATA%\sheet-port\sheet-port.db`; see `docs/development.md` for macOS and
Linux paths and the `SHEET_PORT_DB` override).

## Dev Scripts

| Command | What it does |
|---|---|
| `cargo build --release -p sheet-port-mcp` | Build the MCP sidecar binary |
| `cargo test --workspace` | Rust unit tests for the whole broker |
| `cargo clippy --workspace` | Rust lints |
| `pnpm dev` | Frontend packages in watch mode (parallel) |
| `pnpm build` | Build the TS packages and the frontend |
| `pnpm typecheck` | Strict TypeScript, no emit |
| `pnpm test` | Frontend vitest + MCP e2e smoke (`scripts/e2e-smoke.mjs`) |
| `pnpm test:e2e` | MCP e2e smoke only (needs `cargo build -p sheet-port-mcp`) |
| `pnpm lint` / `pnpm format` | Lint / Prettier |
| `pnpm --filter @sheet-port/desktop dev` | Frontend only at `http://127.0.0.1:8477` (demo fixtures) |
| `pnpm --filter @sheet-port/desktop tauri:dev` | Full desktop app (Rust + React) |

## Security Note

Agents never receive provider OAuth tokens, API keys, raw provider API access, shell
execution, JavaScript execution, or unrestricted writes. The whole broker path is Rust:
tokens stay between the OS keychain and the Rust process, and no npm package ever runs
inside the broker. Every write is a pending change with a diff; changes flagged by
policy require user approval in the desktop app before `commit_change` succeeds,
permissions are re-checked at commit, and everything is audited to SQLite. See
`docs/security.md`.

## Roadmap

- Real Google OAuth in Tauri with OS keychain token storage (next up)
- Functional Google Sheets connector (range-to-record mapping)
- Additional provider connector (bases, field type mapping, rate limits)
- Delete flow with explicit confirmation semantics
- Database encryption at rest
- UI polish: approval notifications, policy presets, richer diff views
