# Airtable - Sheet Port

A local broker that gives AI agents typed, audited access to Google Sheets.

Airtable - Sheet Port is a Tauri desktop app plus a Rust MCP sidecar (`sheet-port-mcp`).
Agents talk only to the sidecar's 18 MCP tools (read, search, stage and commit writes,
format, create and delete tabs). Google access goes through an Apps Script bridge you
deploy on your own account, so there is no Cloud Console project and no OAuth client to
set up. Every write goes through a staged change with a diff and lands in the audit log.

- **Desktop app:** manage Google bridges, permission rules (read / write / delete per
  source or spreadsheet), change history, audit log, a spreadsheet workbench, MCP
  client registration, and the optional loopback HTTP transport.
- **MCP sidecar:** stdio (default) or `127.0.0.1` HTTP. It reads the OS keychain and the
  shared SQLite database directly, so agents keep working when the desktop app is closed.
- **All Rust:** `crates/sheet-port-core` holds every broker rule and is shared by both
  processes. TypeScript exists only in the desktop UI.

See `docs/architecture.md` for the full picture and `docs/mcp-tools.md` for the tool
reference.

## Install

### Download a release

Grab the installer for your platform from the GitHub Releases page (Windows, Linux,
macOS x64 and arm64). The sidecar binary is bundled next to the app executable, and the
app updates itself from signed releases.

### Build from source

Requirements: Rust stable, Node.js 20+ (24 recommended for the e2e smoke), pnpm 9, and
the Tauri 2 platform prerequisites.

```bash
pnpm install                                  # frontend packages
cargo build --release -p sheet-port-mcp       # MCP sidecar -> target/release/sheet-port-mcp(.exe)
pnpm --filter @sheet-port/desktop tauri:dev   # desktop app (Rust + React)
```

The Tauri build bundles the sidecar as an `externalBin`, which must exist at
`apps/desktop/src-tauri/binaries/sheet-port-mcp-<target-triple>`. `node
scripts/stage-sidecar.mjs` builds and copies it there; `tauri:dev` and `tauri:build` run
it automatically (`pnpm stage:sidecar` in `beforeDevCommand` / `beforeBuildCommand`), so
you only need to run it by hand when building outside Tauri.

## Configure

### 1. Connect Google accounts (bridge)

Each Google account is connected through its own Apps Script web app. In short: create a
standalone project at script.google.com, paste `bridge/Code.gs` and
`bridge/appsscript.json`, run `setup` to get a secret, deploy as a web app (execute as
Me, access Anyone), then paste the `/exec` URL and the secret into **Settings > Google
bridges** in the desktop app. Full steps, updating, rotation and revocation:
[`bridge/README.md`](bridge/README.md).

You can add several bridges. Each account becomes the source `google-sheets:{accountKey}`,
and tools route to the right account automatically when `sourceId` is omitted.

### 2. Connect an MCP client

- **From the app:** the **MCP Clients** card in Settings detects Claude Desktop, Claude
  Code, Cursor, Windsurf, Cline, Antigravity and Codex, and registers the sidecar in
  their config (other servers in the file are preserved).
- **By hand:** copy `examples/claude-desktop-config.json` into your client config and
  point `command` at your `sheet-port-mcp` binary (the release build path, or the copy
  bundled with the installed app).

## Tests

```bash
cargo test --workspace                          # core + sidecar unit tests
cargo build -p sheet-port-mcp --features mock   # debug sidecar with the mock connector
pnpm test                                       # frontend vitest + MCP e2e smoke
pnpm test:e2e                                   # e2e smoke only (scripts/e2e-smoke.mjs)
```

The mock connector is compiled only with the cargo feature `mock`; release builds do not
contain it. The e2e smoke drives the sidecar over stdio against a temporary database
(`SHEET_PORT_DB`) and never touches your real data. More commands in
`docs/development.md`.

## Releases

Pushing a tag runs `.github/workflows/build-release.yml`, which builds the
Windows / Linux / macOS (x64 + arm64) matrix, signs the updater bundles, publishes a
GitHub Release and commits the regenerated `updater.json`.

| Tag | Result |
|---|---|
| `release-v<x.y.z>` | Stable release |
| `develop-v<x.y.z>` | Prerelease |

```bash
git tag release-v2.0.0
git push origin release-v2.0.0
```

Signing key setup is in `docs/development.md`.

## Security

- Agents never see Google tokens, the bridge secret, raw Google APIs, a shell, or SQL.
  Tokens and bridge secrets live in the OS keychain (service `sheet-port`).
- There is **no human approval step inside the broker**. Write tools commit by default
  and return the diff; approving what an agent does is the job of your agent harness
  (for example its tool-permission prompts). Use `dryRun: true` to stage without writing,
  and the permission rules to deny write or delete outright.
- Anyone holding a bridge URL **and** its secret can mint one-hour tokens for that
  Google account. Keep the secret private and rotate it if it leaks.

Full trust model: `docs/security.md`.
