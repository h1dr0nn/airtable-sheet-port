# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [2.0.0] - 2026-09-24

### Added
- Apps Script bridge auth. Paste a web app URL and a secret, with no Cloud Console
  project or OAuth client needed. The bridge files and setup guide are in
  `bridge/`, and the guide is also built into Settings.
- A pool of bridges, one per Google account, with Add, Test and Remove in
  Settings > Google bridges.
- Automatic routing. `sourceId` is optional on every tool. With several accounts,
  the bridge that can open the referenced spreadsheet is used and the choice is
  remembered.
- `list_sheets` tool: the tabs (gid and title) of a spreadsheet.
- `read_cells.range`: reads only an A1 window (`B40:F60`, `A:C`, `5:9`) with real
  sheet row numbers.
- `dryRun` on every write tool, which stages only; `commit_change` applies it later.
- Headless bridge management: `sheet-port-mcp bridge add|list|remove`.
- `pnpm test:live`: an end-to-end smoke against a real bridge and spreadsheet.

### Changed
- **Breaking:** writes apply immediately and return the diff so the agent can
  review its own work. The desktop approval step is gone.
- **Breaking:** tools are renamed:
  - `preview_update_records` to `update_records`
  - `preview_update_cells` to `update_cells`
  - `preview_format_table` to `format_table`
  - `preview_create_spreadsheet` to `create_spreadsheet`
  - `preview_create_sheet` to `create_sheet`
  - `preview_delete_sheet` to `delete_sheet`
- **Breaking:** write tool output is now `{ change, committed, outcome? }`, and
  `requiresConfirmation` is removed.
- `delete_sheet` requires `confirm: true`. New bridge sources allow read, write and
  delete by default.
- Tool descriptions are shorter. The shared rules (tableId forms, routing, dryRun)
  moved to the server instructions.
- A failed commit error names the change id that is still staged, so it can be
  retried with `commit_change`.
- The Changes screen is now a history view. Staged dry-run changes can be discarded.
- Permission presets keep read, write and delete only.

### Removed
- The Google OAuth flow: client id and secret, the JSON import, the loopback
  consent page and legacy account migration.
- The approval gate, the auto-approve setting and permission confirmation lists.
- The provider stub connector and the `provider` source kind and token status.
- The mock connector from release builds. It is now behind the cargo feature
  `mock`, used by tests and the e2e smoke. The browser demo backend loads only
  in Vite dev mode.

## [1.0.5] and earlier

Released from tags `release-v1.0.1` through `release-v1.0.5`. See the git history.
