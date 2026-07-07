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

## MCP Client Auto-Configuration

Instead of hand-editing each agent's config, the desktop app can register the
`sheet-port-mcp` sidecar directly into installed MCP clients. The registry and the
merge/preserve logic live in `crates/sheet-port-core/src/mcp_clients.rs`; the Tauri
shell exposes them through four commands (see `docs/ipc.md` shapes):

- `mcp_detect_clients()` - lists every known client with `installed` (config directory
  exists), `configured` (our entry present), and `detectable` (we can locate its config
  on this OS).
- `mcp_configure_client(id)` - merges an entry named `airtable-sheet-port` into that
  client's server map, preserving every other server. Transport-aware: writes a
  `{command,args}` stdio entry pointing at the resolved release sidecar, or a
  `{type:"sse",url}` http entry (`http://127.0.0.1:{port}/mcp`) when the HTTP transport
  is selected. If the release binary is not built yet the entry still points at the
  expected path and the result reports `binaryExists: false`. Codex is TOML instead of
  JSON: the same entry is written as an `[mcp_servers.airtable-sheet-port]` table
  (`command`+`args` for stdio, `url` for http) via `toml_edit`, preserving the user's
  other tables, top-level keys, and comments.
- `mcp_unregister_client(id)` - removes only our entry, leaving all other servers intact.
- `mcp_configure_all()` - configures every detected, installed, detectable client.

Every configure/unregister writes an audit event (`actor = user`,
`action = mcp_client_configured` / `mcp_client_unregistered`, `metadata.client = <id>`).
Writes create parent directories, keep the file otherwise byte-for-byte, and refuse to
touch a config file that is present but not valid JSON (or, for Codex, not valid TOML).

### Supported clients

| Client | Config file | Shape |
| --- | --- | --- |
| Claude Desktop | `%APPDATA%/Claude/claude_desktop_config.json` (Win), `~/Library/Application Support/Claude/...` (macOS), `~/.config/Claude/...` (Linux) | `mcpServers` |
| Claude Code | `~/.claude.json` | `mcpServers` |
| Cursor | `~/.cursor/mcp.json` | `mcpServers` |
| Windsurf | `~/.codeium/windsurf/mcp_config.json` | `mcpServers` |
| Cline (VS Code) | VS Code `User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json` | `mcpServers` |
| Antigravity | `~/.gemini/config/mcp_config.json` | `mcpServers` |
| Codex | `~/.codex/config.toml` | `mcp_servers` (TOML) |
| VS Code (Copilot) | not yet supported (`detectable = false`) | `servers` |

VS Code Copilot is intentionally left `detectable = false`: its workspace form
(`.vscode/mcp.json`) needs a concrete project root the desktop app does not have, and it
uses a different `servers` key. Enabling it requires a project-root-aware path and a new
config shape - see the `TODO(mcp-clients)` in `mcp_clients.rs`. Cline's path covers the
stable VS Code build only (not Insiders / VSCodium / other forks).

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

## Releases and Auto-Update

The desktop app self-updates through `tauri-plugin-updater`. On launch it silently
checks a static manifest on GitHub; when a newer signed version exists, the sidebar
bottom cluster morphs into an "Update Available" prompt, and Settings gains a manual
"Check for Updates" control. The updater endpoint is pinned in
`apps/desktop/src-tauri/tauri.conf.json`:

```
https://raw.githubusercontent.com/h1dr0nn/airtable-sheet-port/main/updater.json
```

### One-time setup (maintainer)

The signing key is generated once and stored as a repository secret. Only the public
key lives in the repo (`tauri.conf.json` `plugins.updater.pubkey`).

```bash
pnpm --filter @sheet-port/desktop exec tauri signer generate -w sheet-port.key
```

Then in the GitHub repository settings add two Actions secrets:

| Secret | Value |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | Full contents of `sheet-port.key` (the workflow strips the `untrusted comment:` header line automatically) |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | The passphrase you set when generating the key (empty if none) |

Keep `sheet-port.key` out of git.

### Cutting a release

Releases are driven by git tags. The `.github/workflows/build-release.yml` workflow
builds a Windows/Linux/macOS (x64 + arm64) matrix, signs each updater bundle, publishes
a GitHub Release, and commits the regenerated `updater.json` back to `main`
(with `[skip ci]`).

| Tag pattern | Result |
|---|---|
| `release-v<x.y.z>` | Stable release (published, not prerelease) |
| `develop-v<x.y.z>` | Prerelease build |

The `<x.y.z>` suffix is injected into `apps/desktop/package.json` and
`apps/desktop/src-tauri/tauri.conf.json` before building, so bump nothing by hand.

```bash
git tag release-v0.0.2
git push origin release-v0.0.2
```

`updater.json` at the repo root is a seed manifest committed so the endpoint resolves
before the first release; the workflow overwrites its `version`, `pub_date`, and
`platforms` (six keys: `windows-x86_64` + `-nsis`, `linux-x86_64` + `-appimage`,
`darwin-x86_64`, `darwin-aarch64`) on every release.

## Run in Background, Tray, and Window Behavior

The desktop shell uses four native Tauri features, wired in
`apps/desktop/src-tauri/src/lib.rs`:

- **Window state** (`tauri-plugin-window-state`): the main window's position, size, and
  maximized state persist across restarts automatically.
- **Single instance** (`tauri-plugin-single-instance`, registered FIRST): a second launch
  focuses the existing window (`show_main_window`) instead of starting a duplicate
  process.
- **Autostart** (`tauri-plugin-autostart`, `LaunchAgent` on macOS): launch-at-login is
  toggled from Settings through `get_autostart_enabled` / `set_autostart_enabled`
  (backed by `app.autolaunch()`). Capabilities:
  `autostart:allow-enable|disable|is-enabled`.
- **System tray** (`tauri::tray::TrayIconBuilder`, app icon): a menu with "Show Window"
  and "Quit". Tray left-click and "Show Window" restore + focus the window; "Quit" exits.

### Close behavior

The `close_behavior` meta key (core `db::get/set_close_behavior`, validated against
`ask` | `tray` | `quit`, default `ask`) drives `WindowEvent::CloseRequested`:

- `quit` - allow the close (the app exits).
- `tray` - `prevent_close()` + hide the window; the app stays resident in the tray.
- `ask` - `prevent_close()` + emit the `close-requested` event so the frontend shows the
  choice modal, which then calls `window_hide_to_tray` or `window_quit`.

`close_behavior` is included in `get_settings` (`AppSettings.closeBehavior`); autostart is
read separately via `get_autostart_enabled`. The desktop-managed sidecar child is killed
on `WindowEvent::Destroyed`, so a real quit (including the tray/frontend Quit paths) never
leaves an orphan MCP server.

### Managed sidecar transport

`mcp_server_start` spawns the managed sidecar on the configured transport
(`db::get_mcp_config`), pinning it via `SHEET_PORT_MCP_TRANSPORT` +
`SHEET_PORT_MCP_PORT`. Because the child keeps the `mcp_heartbeat` row fresh on both
transports, the "running" status works for stdio as well as http (a single-child guard,
missing-binary error, and kill-on-exit still apply).

## Current Limitations

- No SQLite schema migration mechanism yet (only the idempotent initial schema plus a
  `schema_version` meta key).
- The keyring integration is a stub: `token_status` reads entries but no flow writes
  them.
