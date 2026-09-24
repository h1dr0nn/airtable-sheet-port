# PLAN - v2.0.0: Apps Script bridge pool, no approval gate

Goal: connecting Google should take one paste (bridge URL + secret), agents never
wait on the desktop app, and the tool set is direct enough for an agent to finish
a task in the fewest calls. The app keeps the name "Airtable - Sheet Port".

## Phase 1 - Foundation (core crate)

Goal: the core crate talks to Google only through Apps Script bridges, and writes
no longer have an approval gate.

| Task | Files owned | Parallel |
|---|---|---|
| 1a Bridge auth + pool routing | `crates/sheet-port-core/src/google/**`, `vault.rs`, `sources.rs` | yes |
| 1b Remove the approval gate, remove the provider stub, feature-gate mock | `changes.rs`, `changes_tests.rs`, `permissions*.rs`, `types.rs`, `connectors/**`, `mock_data*.rs`, `test_fixtures.rs`, `constants.rs`, `db.rs`, `Cargo.toml` | yes |

Done when:
- `cargo test -p sheet-port-core` passes
- No OAuth code remains
- Adding a bridge stores `{url, secret, deploymentId}` and a cached token
- `google::resolve_source` routes a spreadsheet link to the right bridge
- `changes::commit` never needs a human approval

## Phase 2 - Features (MCP, desktop, bridge script)

| Task | Files owned | Parallel |
|---|---|---|
| 2a MCP tools: direct writes with `dryRun`, optional `sourceId`, `list_sheets`, `read_cells.range`, `delete_sheet.confirm`, shorter descriptions | `crates/sheet-port-mcp/**`, `scripts/e2e-smoke.mjs` | yes |
| 2b Desktop: bridge pool UI and commands, remove OAuth/approval UI | `apps/desktop/**`, `packages/**` | yes |
| 2c Bridge script files + docs | `bridge/**`, `docs/**` (except PLAN/DECISIONS), `README.md` | yes |

Done when:
- `cargo test --workspace`, `pnpm typecheck`, `pnpm lint` and `pnpm test` all pass
- A live test against a real bridge can read, write, format and route

## Phase 3 - Polish

- Cross review of every diff, fix findings (max 3 rounds)
- Full verification: `cargo clippy --workspace -- -D warnings`, `cargo test --workspace`, `pnpm build`, `pnpm typecheck`, `pnpm lint`, `pnpm test`
- Live smoke through the release sidecar binary against the real bridge
- Version 2.0.0, `CHANGELOG.md`, local tag `release-v2.0.0` (not pushed)

## Progress

- [x] Phase 1: core rewritten (bridge auth and routing, no approval gate, provider stub
  removed, mock behind the `mock` feature). Core tests and clippy pass.
- [x] Phase 2: MCP tools v2 (18 tools), desktop bridge pool UI, bridge files and docs.
  Live smoke passed against the real bridge on 2026-09-24.
- [x] Phase 3: two review passes.
  - Core/MCP: 8 defects fixed, including a token leak through a redirect URL in
    transport errors and a failed commit leaving a change stuck as approved.
  - Desktop: 2 defects fixed, including the bridge secret kept in the React Query
    mutation cache.
  - Round 2: removed dead dependencies, cleaned up workbench items on bridge removal,
    made the bridge-test audit best-effort.
  - Full verification is green: fmt, clippy, 259 Rust tests, 69 vitest tests, e2e
    smoke, typecheck, lint, build.
