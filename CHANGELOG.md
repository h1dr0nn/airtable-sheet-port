# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added
- `sheet-port-mcp --version` (or `-V`) prints the sidecar version. On Windows,
  `sheet-port-mcp.exe` now carries file version info (FileVersion,
  ProductVersion, description), shown in Explorer's Details tab.
- The Dashboard's MCP card lists each running sidecar with its version
  ("PID 1234 · v2.2.1") and warns when an MCP client still runs an older
  sidecar, so you know to restart Claude after an update. Sidecars record
  their version in the heartbeat (database schema_version 5).

### Changed
- The Windows release ships only the NSIS installer (`-setup.exe`), which is
  also what the updater installs. The MSI is no longer built.

### Fixed
- Updating while an MCP client runs the sidecar. The installer used to fail on
  the locked `sheet-port-mcp.exe` and keep the old build. It now renames the
  running copy to `sheet-port-mcp.old-N.exe` and installs the new one; the old
  copies are deleted on the next update, app start or uninstall.

## [2.2.0] - 2026-09-25

### Changed
- Closing the window quits the app. There is no close prompt, tray icon or
  background mode. MCP clients on stdio start their own server, so Claude keeps
  working after the app is closed. On the http transport the server is a child
  of the app and stops when the window closes.
- `conditionalFormats` now replace only existing rules on exactly the same
  range. Overlapping rules are kept, so a whole-row rule on `B10:I21` no longer
  wipes the status and priority rules in columns D and E. Pass
  `replaceIntersecting: true` on `format_table` or `append_records` to delete
  every intersecting rule as before.

### Removed
- The "When Closing the Window" setting, the close dialog, the tray icon and
  menu, and the `set_close_behavior`, `window_hide_to_tray` and `window_quit`
  commands.

### Fixed
- Clearing Activity now leaves the list empty. The clear no longer shows up as
  a new `audit_cleared` entry (it stays in the audit trail for `get_audit_log`)
  and no "Activity cleared" toast is shown.
- `format_table` and `append_records` schemas now list `validations` and
  `conditionalFormats`. Every tool's input schema is inlined (no `$defs` or
  `$ref`), with a description per field and item schemas that show their fields
  instead of `{}`. The tool descriptions and server instructions mention
  dropdowns, checkboxes and color rules.
- `get_table_style` takes an optional `headerRow` (1-based, default 1) for
  document-style sheets whose header is lower down. The sample is the next row,
  and the result reports `headerRow`.

## [2.1.0] - 2026-09-25

### Added
- **Guide tab** with the bridge setup steps, both Apps Script files shown in full
  with Copy buttons, and tips. The script.google.com link opens in the system
  browser through the Tauri opener plugin, limited to that URL.
- Google bridges are managed directly on **Data Sources**.
- `format_table` and `append_records` accept `validations` (native dropdowns and
  checkboxes) and `conditionalFormats`, which replace any existing rules on
  intersecting ranges.
- `list_sheets` reports the spreadsheet's `locale` and `timeZone`, `describe_table`
  reports `locale`, and the server instructions explain `;` separators and comma
  decimals in comma-decimal locales.

### Changed
- Committed write results are leaner: `{ change, committed: true, records?,
  formatError?, created? }`, with the committed change included once instead of
  nested again under `outcome`. `commit_change` returns the same shape.
- Calmer themes:
  - Light no longer uses pure white.
  - Dark is a dimmed graphite with elevation shown by lighter surfaces.
  - All text still meets WCAG AA.
- One **Claude** MCP client entry writes both the Claude Desktop and Claude Code
  configs.
- Titlebar icons share one size, stroke and hit area.

### Removed
- The Changes tab. Writes commit directly, and activity is in the audit log.

### Fixed
- Dialogs and the command palette no longer open offset and then snap to the center.
- Menus, dialogs, tooltips, selects and the Activity panel animate out instead of
  vanishing. Menus grow from their trigger. Selects inside dialogs now render
  above the dialog.
- Toasts:
  - The stack no longer jumps when a toast appears.
  - Expanded cards no longer overlap.
  - A replaced toast restarts its timer.
  - Identical toasts refresh instead of stacking.
  - Reduced motion is fully respected.
- Menus keep an 8px gap from the window edge.
- Titlebar tooltips no longer pop up while a menu is open or after it closes.

## [2.0.0] - 2026-09-24

### Added
- Apps Script bridge auth. Paste a web app URL and a secret, with no Cloud Console
  project or OAuth client needed. The bridge files and setup guide are in
  `bridge/`, and the guide is also built into Data Sources.
- A pool of bridges, one per Google account, with Add, Test and Remove in
  Data Sources.
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
