# Development

## Prerequisites

- Rust toolchain (stable) - the whole broker (core crate, MCP sidecar, Tauri backend)
  is Rust
- Tauri 2 platform prerequisites for desktop builds
- Node.js 20+ and pnpm 9, needed only for the React frontend
  (the e2e smoke script additionally uses the built-in `node:sqlite` module to
  simulate desktop approvals, so running `pnpm test` needs a Node release that ships
  it unflagged; Node 24 is recommended)

## Install

```bash
pnpm install   # frontend packages only; cargo fetches Rust deps on first build
```

## Dev Commands

```bash
cargo build -p sheet-port-mcp               # MCP sidecar (debug, used by the e2e smoke)
cargo build --release -p sheet-port-mcp     # MCP sidecar (release, used by Claude Desktop)
pnpm dev                                    # frontend packages in watch mode (parallel)
pnpm --filter @sheet-port/desktop dev       # frontend only, Vite dev server
pnpm --filter @sheet-port/desktop tauri:dev # full desktop app (Rust + React)
```

The frontend intentionally uses the app-specific local port `8477`
(`http://127.0.0.1:8477`). In a plain browser (without Tauri) the UI falls back to
in-memory demo fixtures; run `tauri:dev` to exercise the real Rust backend, the custom
titlebar (the window uses `decorations: false`), and the shared database.

## Build Commands

```bash
cargo build --workspace   # all Rust crates
pnpm build                # TS packages + frontend
pnpm typecheck            # tsc project references, no emit
cargo clippy --workspace  # Rust lints
pnpm lint
pnpm format
```

## Test Commands

```bash
cargo test --workspace          # all broker logic (sheet-port-core: 78 tests, sheet-port-mcp: 14)
cargo build -p sheet-port-mcp   # build the debug sidecar binary for the e2e smoke
pnpm test                       # frontend vitest + MCP e2e smoke
pnpm test:e2e                   # MCP e2e smoke only
```

The MCP end-to-end smoke (`scripts/e2e-smoke.mjs`) spawns
`target/debug/sheet-port-mcp` (`.exe` on Windows) over stdio against a temp database
(via `SHEET_PORT_DB`) and drives the preview -> approve -> commit enforcement,
including the "commit refused without approval" path and the heartbeat row. It fails
fast with a clear message when the binary is missing, so run
`cargo build -p sheet-port-mcp` first. Rust tests use isolated temp-file databases and
never touch your real data.

## Shared Database

Both processes open the same SQLite file (WAL mode). Locations:

| OS | Path |
|---|---|
| Windows | `%APPDATA%\sheet-port\sheet-port.db` |
| macOS | `~/Library/Application Support/sheet-port/sheet-port.db` |
| Linux | `$XDG_DATA_HOME/sheet-port/sheet-port.db`, else `~/.local/share/sheet-port/sheet-port.db` |

Override with the `SHEET_PORT_DB` environment variable (absolute file path). This is how
tests and smoke scripts isolate state. Whichever process opens the DB first applies
`crates/sheet-port-core/sql/schema.sql` and `crates/sheet-port-core/sql/seed.sql`
(embedded via `include_str!` in `db.rs`; the schema is idempotent and the seed is
guarded by the `meta` key `seeded`). To reset local state, stop both processes and
delete the DB file (plus its `-wal`/`-shm` siblings).

`docs/ipc.md` is the canonical contract for the Tauri commands and the shared-state
model; keep `apps/desktop/src/lib/ipc.ts` and `crates/sheet-port-core/src/types.rs` in
sync with it.

## How to Run the MCP Server

```bash
cargo build --release -p sheet-port-mcp
./target/release/sheet-port-mcp        # sheet-port-mcp.exe on Windows; stdio transport
```

For Claude Desktop use `examples/claude-desktop-config.json`; it points at the release
binary, so build it first. stdout belongs to the MCP transport; diagnostics go to
stderr.

## How to Add a New MCP Tool

1. Define the input model in `crates/sheet-port-mcp/src/args.rs`: a `Deserialize` +
   `JsonSchema` struct with `rename_all = "camelCase"` and a `validate` method that
   enforces the contract bounds (follow the existing limits: list sizes 1-100, page
   limits 1-500, query strings capped at 200 chars). Reuse the constants from
   `sheet_port_core::constants`.
2. Implement the tool in `crates/sheet-port-mcp/src/tools.rs`. Check permissions first
   through `sheet_port_core::permissions` (`assert_can_read` / `assert_can_write`).
   Writes must go through `sheet_port_core::changes` (preview + commit), never directly
   through a connector.
3. Route data access through the `ConnectorRegistry` passed into the closure (never a
   concrete connector).
4. Record an audit event through `sheet_port_core::audit::record` with actor
   `AuditActor::Agent` and useful metadata.
5. Register the tool in `crates/sheet-port-mcp/src/server.rs` with a `#[tool(...)]`
   method; add `annotations(read_only_hint = true)` for read-only tools.
6. Add unit tests in `tools_tests.rs` / `args_tests.rs` and document the tool in
   `docs/mcp-tools.md` (input bounds, output shape, permission, example call and
   response).

## How to Add a New Connector

1. Add a module under `crates/sheet-port-core/src/connectors/` implementing the
   `TableConnector` trait (`connectors/mod.rs`) with a unique `kind`.
2. If the kind is new, add a variant to `SourceKind` in
   `crates/sheet-port-core/src/types.rs` and extend the `sources.kind` CHECK
   constraint in `crates/sheet-port-core/sql/schema.sql`.
3. Register it in `ConnectorRegistry::with_default_connectors`
   (`connectors/mod.rs`). The registry routes by the `sources.kind` column, so a
   source row with your kind is all the wiring the router needs.
4. Keep credentials inside the OS keychain (service `sheet-port`, see `vault.rs`).
   Connectors must never receive tokens through MCP tool inputs.
5. Add provider mapping notes to `docs/connectors.md`.

## Current Limitations

- No SQLite schema migration mechanism yet (only the idempotent initial schema plus a
  `schema_version` meta key).
- The keyring integration is a stub: `token_status` reads entries but no flow writes
  them.
- The desktop app does not manage the sidecar lifecycle; MCP clients spawn the server.
