import type { Language } from "../lib/ipc.js";

/**
 * Typed translation dictionary keyed by dot-namespaced string id. `en` is the
 * source of truth and lists every user-facing string; `vi` mirrors the same
 * keys with natural Vietnamese. The `TranslationKey` union is derived from `en`
 * so a missing `vi` key surfaces at typecheck time.
 *
 * Placeholders use {name} syntax and are interpolated by useTranslation's t().
 * Product name, credit, technical ids/paths, provider wire values, and code are
 * intentionally NOT translated.
 */
export const en = {
  // Shared / generic
  "common.cancel": "Cancel",
  "common.close": "Close",
  "common.save": "Save",
  "common.saving": "Saving...",
  "common.working": "Working...",
  "common.loading": "Loading...",
  "common.noChangesToSave": "No changes to save",
  "common.running": "Running",
  "common.offline": "Offline",
  "common.connected": "Connected",
  "common.notConnected": "Not connected",

  // Screen headers
  "screen.dashboard.title": "Dashboard",
  "screen.dashboard.description": "Local capability broker between agents and your spreadsheets",
  "screen.sources.title": "Data Sources",
  "screen.sources.description":
    "Connect table providers here; agents only ever see what permission rules allow",
  "screen.tables.title": "Tables",
  "screen.tables.description": "Browse records through the same read path agents use",
  "screen.changes.title": "Changes",
  "screen.changes.description":
    "History of agent writes. Staged dry runs wait here until the agent commits or you discard them",
  "screen.settings.title": "Settings",
  "screen.settings.description":
    "Appearance, connections, permissions, and application details",

  // Navigation labels
  "nav.dashboard": "Dashboard",
  "nav.sources": "Data Sources",
  "nav.tables": "Tables",
  "nav.changes": "Changes",
  "nav.settings": "Settings",

  // Titlebar
  "titlebar.menu": "Menu",
  "titlebar.applicationMenu": "Application menu",
  "titlebar.search": "Search",
  "titlebar.activity": "Activity",
  "titlebar.expandSidebar": "Expand Sidebar",
  "titlebar.collapseSidebar": "Collapse Sidebar",
  "titlebar.minimize": "Minimize",
  "titlebar.minimizeWindow": "Minimize window",
  "titlebar.maximize": "Maximize",
  "titlebar.toggleMaximize": "Toggle maximize",
  "titlebar.close": "Close",
  "titlebar.closeWindow": "Close window",
  "titlebar.commandPalette": "Open command palette",

  // Titlebar menu (lib/menu.ts)
  "menu.file": "File",
  "menu.reloadData": "Reload Data",
  "menu.quit": "Quit",
  "menu.view": "View",
  "menu.theme": "Theme",
  "menu.help": "Help",
  "menu.about": "About",
  "menu.copyVersion": "Copy Version",

  // Theme labels (shared by menu, command palette, appearance)
  "theme.light": "Light",
  "theme.dark": "Dark",
  "theme.system": "System",

  // Copy button
  "copy.copy": "Copy",
  "copy.copied": "Copied",
  "copy.failed": "Copy failed",

  // Titlebar toasts
  "titlebar.reloadingData": "Reloading data",
  "titlebar.versionUnavailable": "Version unavailable",
  "titlebar.versionUnavailableDesc": "App status has not loaded yet",
  "titlebar.versionCopied": "Version copied",
  "titlebar.copyFailed": "Copy failed",

  // Command palette
  "palette.placeholder": "Type a command or search...",
  "palette.noResults": "No results found",
  "palette.screens": "Screens",
  "palette.tables": "Tables",
  "palette.actions": "Actions",
  "palette.screen": "Screen",
  "palette.themeLight": "Theme: Light",
  "palette.themeDark": "Theme: Dark",
  "palette.themeSystem": "Theme: System",
  "palette.addGoogleBridge": "Add Google Bridge",

  // Activity dropdown
  "activity.title": "Activity",
  "activity.clear": "Clear",
  "activity.loadMore": "Load More",
  "activity.loading": "Loading...",
  "activity.emptyTitle": "No Activity Yet",
  "activity.emptyDescription": "Agent activity is recorded here as soon as it happens",
  "activity.expandMetadata": "Expand metadata",
  "activity.collapseMetadata": "Collapse metadata",

  // Dashboard cards
  "dashboard.mcpServer": "MCP Server",
  "dashboard.mcpOfflineHint":
    "Register Sheet Port with your MCP client from Settings, then restart the client.",
  "dashboard.pendingApprovals": "Pending Changes",
  "dashboard.nothingWaiting": "No staged changes",
  "dashboard.oneChangeAwaiting": "Staged change not committed yet",
  "dashboard.changesAwaiting": "Staged changes not committed yet",
  "dashboard.reviewChanges": "View Changes",
  "dashboard.database": "Database",
  "dashboard.copyDatabasePath": "Copy database path",
  "dashboard.sharedSqlite": "Shared SQLite",
  "dashboard.version": "Version",
  "dashboard.tokenVault": "Token Vault",
  "dashboard.googleSheets": "Google Sheets",
  "dashboard.inKeychain": "In Keychain",
  "dashboard.notStored": "Not Stored",
  "dashboard.tokensNeverLeave": "Bridge secrets and tokens never leave the OS keychain.",
  "dashboard.noSourcesTitle": "No Data Sources Connected",
  "dashboard.noSourcesDescription":
    "Connect a data source such as Google Sheets to give agents something to read.",
  "dashboard.connectDataSource": "Connect a Data Source",
  "dashboard.recentActivity": "Recent Activity",
  "dashboard.recentActivityEmpty": "Agent activity shows up here as it happens.",
  "dashboard.recentChanges": "Recent Changes",
  "dashboard.viewAll": "View All",
  "dashboard.recentChangesEmpty": "Agent writes show up here as they happen.",

  // Data Sources
  "sources.googleSheets": "Google Sheets",
  "sources.disconnect": "Disconnect",
  "sources.disconnectTooltip": "Remove this account's bridge and its stored secret",
  "sources.linkedTo": "Linked to {email}",
  "sources.disconnectTitle": "Disconnect Google Account?",
  "sources.disconnectDescription":
    "Agents lose access to this account's spreadsheets, and its bridge URL and secret are removed from the OS keychain. You can add the bridge again at any time.",
  "sources.addGoogleAccount": "Add Google Account",
  "sources.addGoogleAccountHint":
    "Add an Apps Script bridge in Settings to link another Google account",
  "sources.statusConnected": "Connected",
  "sources.statusPlaceholder": "Placeholder",
  "sources.statusError": "Error",
  "sources.genericConnected": "Available to agents through permission rules",
  "sources.genericPlaceholder":
    "Connector scaffolded; authentication is not wired up yet",

  // Workbench
  "workbench.title": "Workbench",
  "workbench.addMenu": "Add to Workbench",
  "workbench.newFolder": "New Folder",
  "workbench.addSpreadsheet": "Add Spreadsheet",
  "workbench.add": "Add",
  "workbench.create": "Create",
  "workbench.searchPlaceholder": "Search spreadsheets",
  "workbench.ungrouped": "Ungrouped",
  "workbench.rename": "Rename",
  "workbench.renameFolder": "Rename Folder",
  "workbench.delete": "Delete",
  "workbench.remove": "Remove",
  "workbench.moveToFolder": "Move to Folder",
  "workbench.folderMenu": "{name} folder actions",
  "workbench.itemMenu": "{name} actions",
  "workbench.emptyFolder": "No spreadsheets",
  "workbench.emptyTitle": "No Spreadsheets Yet",
  "workbench.emptyDescription":
    "Add a Google Sheets spreadsheet to start building your workspace.",
  "workbench.noResults": "No matches",
  "workbench.folderNameLabel": "Folder Name",
  "workbench.folderNamePlaceholder": "e.g. Game Config",
  "workbench.addSpreadsheetDescription":
    "Paste a Google Sheets link or spreadsheet id, then pick a folder.",
  "workbench.spreadsheetUrlLabel": "Spreadsheet URL or ID",
  "workbench.spreadsheetUrlPlaceholder": "https://docs.google.com/spreadsheets/d/...",
  "workbench.folderLabel": "Folder",
  "workbench.deleteFolderTitle": "Delete Folder?",
  "workbench.deleteFolderDescription":
    "\"{name}\" is deleted and its spreadsheets move to Ungrouped. The spreadsheets themselves are not removed.",
  "workbench.removeItemTitle": "Remove Spreadsheet?",
  "workbench.removeItemDescription":
    "\"{name}\" is removed from the Workbench. The spreadsheet itself is not deleted.",
  "workbench.findInSheet": "Find in sheet",
  "workbench.refresh": "Refresh",
  "workbench.addRow": "Add Row",
  "workbench.undo": "Undo",
  "workbench.redo": "Redo",
  "workbench.sheetTabs": "Sheet Tabs",
  "workbench.selectPromptTitle": "Select or add a spreadsheet",
  "workbench.selectPromptDescription":
    "Choose a spreadsheet from the left, or add one to get started.",
  "workbench.sheetLoadError": "This sheet could not be loaded.",

  // Changes
  "changes.filterAll": "All",
  "changes.filterPending": "Pending",
  "changes.filterApproved": "Approved",
  "changes.filterCommitted": "Committed",
  "changes.filterRejected": "Discarded",
  "changes.filterAria": "Filter changes by status",
  "changes.emptyAll": "No Changes Yet",
  "changes.emptyFiltered": "No {filter} Changes",
  "changes.emptyDescription": "Agent writes are recorded here with a diff of what changed",
  "changes.stagedDryRun": "Staged dry run · not committed yet",
  "changes.discard": "Discard",
  "changes.discarding": "Discarding...",
  "changes.statusPending": "Pending",
  "changes.statusApproved": "Approved",
  "changes.statusCommitted": "Committed",
  "changes.statusRejected": "Discarded",
  "changes.committedBy": "Committed {time} by {who}",
  "changes.committed": "Committed {time}",
  "changes.rejected": "Discarded {time}",
  "changes.approvedWaiting": "Approved {time} · waiting for the agent to commit",
  "changes.recordLabel": "Record {id}",
  "changes.formatCellsHeading": "Cell formatting",
  "changes.formatLayoutHeading": "Sheet layout",
  "changes.formatFreezeRows": "Freeze first {count} row(s)",
  "changes.formatFreezeColumns": "Freeze first {count} column(s)",
  "changes.formatColumnWidth": "Column {column}: {pixels}px",

  // Records table
  "records.record": "Record",
  "records.previous": "Previous",
  "records.next": "Next",
  "records.range": "{first}-{last} of {total} records",

  // Settings - Appearance
  "settings.appearance.title": "Appearance",
  "settings.appearance.theme": "Theme",
  "settings.appearance.themeFollowsSystem": "Follows your system preference (currently {mode})",
  "settings.appearance.themeFixed": "Fixed for this device",
  "settings.appearance.fontSize": "Font Size",
  "settings.appearance.fontSizeDescription": "Scales the whole interface up or down.",
  "settings.appearance.fontSizeSmall": "Small",
  "settings.appearance.fontSizeNormal": "Normal",
  "settings.appearance.fontSizeLarge": "Large",
  "settings.appearance.font": "Font",
  "settings.appearance.fontDescription":
    "Classic is a serif face, Modern is Inter, System uses your OS UI font.",
  "settings.appearance.fontClassic": "Classic",
  "settings.appearance.fontModern": "Modern",
  "settings.appearance.fontSystem": "System",
  "settings.appearance.language": "Language",
  "settings.appearance.languageDescription": "Choose the language for the interface.",
  "settings.appearance.languageEnglish": "English",
  "settings.appearance.languageVietnamese": "Vietnamese",
  "settings.bridges.title": "Google Bridges",
  "settings.bridges.description":
    "Each bridge is a small Apps Script web app deployed on one Google account. It hands out short-lived access tokens, so no Cloud Console project or OAuth client is needed.",
  "settings.bridges.empty": "No bridges yet. Add one below to connect a Google account.",
  "settings.bridges.deployment": "Deployment",
  "settings.bridges.missingCredential":
    "No bridge is stored for this account. Remove it, then add its bridge again.",
  "settings.bridges.test": "Test",
  "settings.bridges.testing": "Testing...",
  "settings.bridges.remove": "Remove",
  "settings.bridges.removeTitle": "Remove Bridge?",
  "settings.bridges.removeDescription":
    "Agents lose access to {email}'s spreadsheets, and the bridge URL and secret are removed from the OS keychain. The Apps Script deployment itself is not changed.",
  "settings.bridges.addTitle": "Add a Bridge",
  "settings.bridges.url": "Web App URL",
  "settings.bridges.secret": "Secret",
  "settings.bridges.secretPlaceholder": "Value logged by setup()",
  "settings.bridges.secretHint":
    "Stored in the OS keychain together with the URL; it is only ever sent to your bridge.",
  "settings.bridges.add": "Add Bridge",
  "settings.bridges.adding": "Adding...",
  "settings.bridges.guideToggle": "How to create a bridge",
  "settings.bridges.step1":
    "Open script.google.com with the Google account agents should use and create a new project.",
  "settings.bridges.step2":
    "In Project Settings, tick \"Show appsscript.json manifest file in editor\".",
  "settings.bridges.step3":
    "Replace the contents of Code.gs and appsscript.json with the two files below.",
  "settings.bridges.step4":
    "Select the setup function and click Run, allow the permissions, then copy the SECRET value from the execution log.",
  "settings.bridges.step5":
    "Click Deploy > New deployment, choose Web app with Execute as: Me and Who has access: Anyone, then copy the web app URL ending in /exec.",
  "settings.bridges.step6": "Paste the URL and the secret above and click Add Bridge.",
  "settings.bridges.copyCode": "Copy Code.gs",
  "settings.bridges.copyManifest": "Copy appsscript.json",

  // Settings - Google Sheets

  // Settings - Google JSON import modal

  // Settings - MCP Server
  "settings.mcpServer.title": "MCP Server",
  "settings.mcpServer.transport": "Transport",
  "settings.mcpServer.transportDescription":
    "Stdio spawns the sidecar per client; Local HTTP serves one shared endpoint. After app updates, Local HTTP picks up new tools automatically, while stdio clients must be restarted.",
  "settings.mcpServer.transportAria": "MCP Transport",
  "settings.mcpServer.transportStdio": "Stdio",
  "settings.mcpServer.transportHttp": "Local HTTP",
  "settings.mcpServer.httpPort": "HTTP Port",
  "settings.mcpServer.httpPortHint": "Loopback port for the local HTTP endpoint. Range {min}-{max}.",
  "settings.mcpServer.enterPort": "Enter a port",
  "settings.mcpServer.portWholeNumber": "Port must be a whole number",
  "settings.mcpServer.portRange": "Port must be between {min} and {max}",
  "settings.mcpServer.serverProcess": "Server Process",
  "settings.mcpServer.serverProcessHttp":
    "Runs the shared HTTP endpoint as a desktop-managed process.",
  "settings.mcpServer.serverProcessStdio":
    "MCP clients usually launch their own instance; this runs a local managed one.",
  "settings.mcpServer.start": "Start",
  "settings.mcpServer.starting": "Starting...",
  "settings.mcpServer.stop": "Stop",
  "settings.mcpServer.stopping": "Stopping...",
  "settings.mcpServer.endpointUrl": "Endpoint URL",
  "settings.mcpServer.copyEndpoint": "Copy endpoint URL",
  "settings.mcpServer.restartHint":
    "Changing the transport or port requires restarting the sidecar to take effect.",
  "settings.mcpServer.configuring": "Configuring...",

  // Settings - MCP Clients
  "settings.mcpClients.title": "MCP Clients",
  "settings.mcpClients.client": "Client",
  "settings.mcpClients.selectClient": "Select a client",
  "settings.mcpClients.clientAria": "MCP Client",
  "settings.mcpClients.noneDetected": "No supported MCP clients detected",
  "settings.mcpClients.configure": "Configure",
  "settings.mcpClients.configuring": "Configuring...",
  "settings.mcpClients.unregister": "Unregister",
  "settings.mcpClients.configureAll": "Configure All Detected Clients",
  "settings.mcpClients.configFile": "Config file",
  "settings.mcpClients.notInstalled": "{name} is not installed",
  "settings.mcpClients.alreadyConfigured": "Already configured",
  "settings.mcpClients.noneNeedConfigure": "No detected clients need configuring",
  "settings.mcpClients.stateConfigured": "Configured",
  "settings.mcpClients.stateMissingConfig": "Missing Config",
  "settings.mcpClients.stateNotFound": "Not Found",
  "settings.mcpClients.stateUnknown": "Unknown",
  "settings.mcpClients.unregisterTitle": "Unregister From {name}?",
  "settings.mcpClients.unregisterDescription":
    "This edits {name}'s config file to remove the Sheet Port MCP server. You can reconfigure it at any time.",

  // Settings - Permissions
  "settings.permissions.title": "Permissions",
  "settings.permissions.connectFirst": "Connect a data source first",
  "settings.permissions.hint":
    "Pick an access preset per source. Allowed agent writes apply directly and are recorded in Changes.",
  "settings.permissions.presetAria": "Permission preset for {name}",
  "settings.permissions.custom": "Custom",
  "settings.permissions.customHint":
    "This source uses a custom rule. Pick a preset to normalize it.",
  "settings.permissions.updated": "Updated {time}",
  "settings.permissions.updatedPrefix": "Updated",
  "settings.permissions.bypassTitle": "Bypass Permission?",
  "settings.permissions.bypassDescription":
    "Agents get full access to this source, including deletes. Only choose this if you fully trust every connected agent.",
  "settings.permissions.enableBypass": "Enable Bypass",

  // Permission presets (lib/permissionPresets.ts)
  "preset.readOnly.label": "Read Only",
  "preset.readOnly.description":
    "Agents can read records but cannot write, update, or delete.",
  "preset.readWrite.label": "Read & Write",
  "preset.readWrite.description":
    "Agents can read, append, and update records. Deletes stay blocked.",
  "preset.bypass.label": "Bypass Permission",
  "preset.bypass.description":
    "Full access, including deletes.",

  // Settings - About
  "settings.about.title": "About",
  "settings.about.checkUpdates": "Check for Updates",
  "settings.about.checking": "Checking...",
  "settings.about.appName": "App Name",
  "settings.about.version": "Version",
  "settings.about.createdBy": "Created By",
  "settings.about.database": "Database",
  "settings.about.upToDate": "You're on the latest version",
  "settings.about.updateCheckFailed": "Update check failed",
  "settings.about.updateAvailableTitle": "Update Available",
  "settings.about.updateAvailableVersion":
    "Version {version} will be downloaded and installed. The app will restart to finish.",
  "settings.about.updateAvailableGeneric":
    "A newer version will be downloaded and installed. The app will restart to finish.",
  "settings.about.releaseNotes": "Release Notes",
  "settings.about.install": "Install",
  "settings.about.installing": "Installing...",

  // Settings - General
  "settings.general.title": "General",
  "settings.general.whenClosing": "When Closing the Window",
  "settings.general.whenClosingDescription":
    "Ask each time, keep running in the tray, or quit the app.",
  "settings.general.closeAsk": "Ask",
  "settings.general.closeTray": "Run in Background",
  "settings.general.closeQuit": "Quit",
  "settings.general.launchAtLogin": "Launch at Login",
  "settings.general.launchAtLoginDescription":
    "Start the app automatically when you sign in.",

  // Settings - Reset
  "settings.reset.title": "Reset",
  "settings.reset.description":
    "Restore preferences to their defaults. Your Google bridges, permission rules, and data are not affected.",
  "settings.reset.button": "Reset to Default",
  "settings.reset.confirmTitle": "Reset to Default?",
  "settings.reset.confirmDescription":
    "Theme, font, and language return to their defaults. This does NOT remove your Google bridges, permission rules, or data.",

  // Close behavior dialog
  "closeDialog.title": "Run in Background?",
  "closeDialog.description": "Keep the app running in the system tray, or quit it entirely.",
  "closeDialog.rememberChoice": "Remember My Choice",
  "closeDialog.quit": "Quit",
  "closeDialog.quitting": "Quitting...",
  "closeDialog.runInBackground": "Run in Background",
  "closeDialog.minimizing": "Minimizing...",

  // Sidebar update card
  "sidebar.updateAvailable": "Update Available",
  "sidebar.updateAvailableVersion": "Update Available: v{version}",
  "sidebar.downloadingUpdate": "Downloading Update...",
  "sidebar.downloading": "Downloading...",
  "sidebar.update": "Update",

  // Empty states
  "empty.records.title": "No Records",
  "empty.records.description":
    "This table is empty. Agent appends will show up here after commit",

  // Toasts - settings
  "toast.updatedRestartClients":
    "Updated to v{version}. Restart your MCP clients (Claude Desktop, Claude Code, ...) to load the new tools.",
  "toast.fontSizeError": "Font size not updated",
  "toast.fontError": "Font not updated",
  "toast.languageError": "Language not updated",
  "toast.languageUpdated": "Language updated",
  "toast.resetFailed": "Reset failed",
  "toast.settingsReset": "Settings reset to default",
  "toast.closeBehaviorError": "Close behavior not updated",
  "toast.launchAtLoginError": "Launch at login not updated",
  "toast.launchAtLoginEnabled": "Launch at login enabled",
  "toast.launchAtLoginDisabled": "Launch at login disabled",
  "toast.bridgeAdded": "Google bridge added",
  "toast.bridgeAddError": "Bridge not added",
  "toast.bridgeSignedInAs": "Signed in as {email}",
  "toast.bridgeTestOk": "Bridge is working",
  "toast.bridgeTestError": "Bridge test failed",
  "toast.bridgeRemoved": "Google bridge removed",
  "toast.bridgeRemoveError": "Bridge not removed",

  // Toasts - Google

  // Toasts - MCP
  "toast.transportError": "Transport not updated",
  "toast.transportSaved": "MCP transport saved",
  "toast.restartToApply": "Restart the sidecar to apply",
  "toast.portError": "Port not saved",
  "toast.portSaved": "MCP port saved",
  "toast.clientConfigError": "Client not configured",
  "toast.clientConfigured": "MCP client configured",
  "toast.clientUnregisterError": "Client not unregistered",
  "toast.clientUnregistered": "MCP client unregistered",
  "toast.serverStartError": "MCP server not started",
  "toast.serverStarted": "MCP server started",
  "toast.serverStopError": "MCP server not stopped",
  "toast.serverStopped": "MCP server stopped",
  "toast.clientsConfigError": "Clients not configured",
  "toast.clientsConfigured": "Detected MCP clients configured",
  "toast.changeDiscarded": "Change discarded",
  "toast.changeDiscardError": "Change not discarded",

  // Toasts - Changes

  // Toasts - Workbench
  "toast.folderCreated": "Folder created",
  "toast.folderCreateError": "Folder not created",
  "toast.folderRenamed": "Folder renamed",
  "toast.folderRenameError": "Folder not renamed",
  "toast.folderDeleted": "Folder deleted",
  "toast.folderDeleteError": "Folder not deleted",
  "toast.spreadsheetAdded": "Spreadsheet added",
  "toast.spreadsheetAddError": "Spreadsheet not added",
  "toast.spreadsheetRemoved": "Spreadsheet removed",
  "toast.spreadsheetRemoveError": "Spreadsheet not removed",
  "toast.spreadsheetMoved": "Spreadsheet moved",
  "toast.spreadsheetMoveError": "Spreadsheet not moved",
  "toast.cellUpdateError": "Cell not updated",
  "toast.rowAdded": "Row added",
  "toast.rowAddError": "Row not added"
} as const;

export type TranslationKey = keyof typeof en;

/** Every dictionary must supply the same keys as `en` (enforced by the type). */
type Dictionary = Record<TranslationKey, string>;

const vi: Dictionary = {
  // Shared / generic
  "common.cancel": "Hủy",
  "common.close": "Đóng",
  "common.save": "Lưu",
  "common.saving": "Đang lưu...",
  "common.working": "Đang xử lý...",
  "common.loading": "Đang tải...",
  "common.noChangesToSave": "Không có thay đổi để lưu",
  "common.running": "Đang chạy",
  "common.offline": "Ngoại tuyến",
  "common.connected": "Đã kết nối",
  "common.notConnected": "Chưa kết nối",

  // Screen headers
  "screen.dashboard.title": "Tổng quan",
  "screen.dashboard.description":
    "Cầu nối năng lực cục bộ giữa các agent và bảng tính của bạn",
  "screen.sources.title": "Nguồn dữ liệu",
  "screen.sources.description":
    "Kết nối các nhà cung cấp bảng tại đây; agent chỉ thấy những gì quy tắc quyền cho phép",
  "screen.tables.title": "Bảng",
  "screen.tables.description":
    "Duyệt bản ghi qua cùng luồng đọc mà agent sử dụng",
  "screen.changes.title": "Thay đổi",
  "screen.changes.description":
    "Lịch sử thao tác ghi của agent. Bản chạy thử được lưu tại đây cho đến khi agent commit hoặc bạn hủy bỏ",
  "screen.settings.title": "Cài đặt",
  "screen.settings.description":
    "Giao diện, kết nối, quyền và thông tin ứng dụng",

  // Navigation labels
  "nav.dashboard": "Tổng quan",
  "nav.sources": "Nguồn dữ liệu",
  "nav.tables": "Bảng",
  "nav.changes": "Thay đổi",
  "nav.settings": "Cài đặt",

  // Titlebar
  "titlebar.menu": "Menu",
  "titlebar.applicationMenu": "Menu ứng dụng",
  "titlebar.search": "Tìm kiếm",
  "titlebar.activity": "Hoạt động",
  "titlebar.expandSidebar": "Mở rộng thanh bên",
  "titlebar.collapseSidebar": "Thu gọn thanh bên",
  "titlebar.minimize": "Thu nhỏ",
  "titlebar.minimizeWindow": "Thu nhỏ cửa sổ",
  "titlebar.maximize": "Phóng to",
  "titlebar.toggleMaximize": "Bật/tắt phóng to",
  "titlebar.close": "Đóng",
  "titlebar.closeWindow": "Đóng cửa sổ",
  "titlebar.commandPalette": "Mở bảng lệnh",

  // Titlebar menu
  "menu.file": "Tệp",
  "menu.reloadData": "Tải lại dữ liệu",
  "menu.quit": "Thoát",
  "menu.view": "Xem",
  "menu.theme": "Giao diện",
  "menu.help": "Trợ giúp",
  "menu.about": "Giới thiệu",
  "menu.copyVersion": "Sao chép phiên bản",

  // Theme labels
  "theme.light": "Sáng",
  "theme.dark": "Tối",
  "theme.system": "Hệ thống",

  // Copy button
  "copy.copy": "Sao chép",
  "copy.copied": "Đã sao chép",
  "copy.failed": "Sao chép thất bại",

  // Titlebar toasts
  "titlebar.reloadingData": "Đang tải lại dữ liệu",
  "titlebar.versionUnavailable": "Không có phiên bản",
  "titlebar.versionUnavailableDesc": "Trạng thái ứng dụng chưa được tải",
  "titlebar.versionCopied": "Đã sao chép phiên bản",
  "titlebar.copyFailed": "Sao chép thất bại",

  // Command palette
  "palette.placeholder": "Nhập lệnh hoặc tìm kiếm...",
  "palette.noResults": "Không tìm thấy kết quả",
  "palette.screens": "Màn hình",
  "palette.tables": "Bảng",
  "palette.actions": "Hành động",
  "palette.screen": "Màn hình",
  "palette.themeLight": "Giao diện: Sáng",
  "palette.themeDark": "Giao diện: Tối",
  "palette.themeSystem": "Giao diện: Hệ thống",
  "palette.addGoogleBridge": "Thêm cầu nối Google",

  // Activity dropdown
  "activity.title": "Hoạt động",
  "activity.clear": "Xóa",
  "activity.loadMore": "Tải thêm",
  "activity.loading": "Đang tải...",
  "activity.emptyTitle": "Chưa có hoạt động",
  "activity.emptyDescription": "Hoạt động của agent được ghi lại tại đây ngay khi xảy ra",
  "activity.expandMetadata": "Mở rộng metadata",
  "activity.collapseMetadata": "Thu gọn metadata",

  // Dashboard cards
  "dashboard.mcpServer": "Máy chủ MCP",
  "dashboard.mcpOfflineHint":
    "Đăng ký Sheet Port với client MCP của bạn từ Cài đặt, sau đó khởi động lại client.",
  "dashboard.pendingApprovals": "Thay đổi đang chờ",
  "dashboard.nothingWaiting": "Không có bản chạy thử nào",
  "dashboard.oneChangeAwaiting": "Bản chạy thử chưa được commit",
  "dashboard.changesAwaiting": "Bản chạy thử chưa được commit",
  "dashboard.reviewChanges": "Xem thay đổi",
  "dashboard.database": "Cơ sở dữ liệu",
  "dashboard.copyDatabasePath": "Sao chép đường dẫn cơ sở dữ liệu",
  "dashboard.sharedSqlite": "SQLite dùng chung",
  "dashboard.version": "Phiên bản",
  "dashboard.tokenVault": "Kho token",
  "dashboard.googleSheets": "Google Sheets",
  "dashboard.inKeychain": "Trong keychain",
  "dashboard.notStored": "Chưa lưu",
  "dashboard.tokensNeverLeave": "Secret của cầu nối và token không bao giờ rời khỏi keychain của hệ điều hành.",
  "dashboard.noSourcesTitle": "Chưa kết nối nguồn dữ liệu",
  "dashboard.noSourcesDescription":
    "Kết nối một nguồn dữ liệu như Google Sheets để agent có thể đọc.",
  "dashboard.connectDataSource": "Kết nối nguồn dữ liệu",
  "dashboard.recentActivity": "Hoạt động gần đây",
  "dashboard.recentActivityEmpty": "Hoạt động của agent hiển thị tại đây khi xảy ra.",
  "dashboard.recentChanges": "Thay đổi gần đây",
  "dashboard.viewAll": "Xem tất cả",
  "dashboard.recentChangesEmpty": "Thao tác ghi của agent xuất hiện tại đây ngay khi diễn ra.",

  // Data Sources
  "sources.googleSheets": "Google Sheets",
  "sources.disconnect": "Ngắt kết nối",
  "sources.disconnectTooltip": "Xóa cầu nối của tài khoản này và secret đã lưu",
  "sources.linkedTo": "Liên kết với {email}",
  "sources.disconnectTitle": "Ngắt kết nối tài khoản Google?",
  "sources.disconnectDescription":
    "Agent mất quyền truy cập vào bảng tính của tài khoản này, và URL cùng secret của cầu nối sẽ bị xóa khỏi keychain của hệ điều hành. Bạn có thể thêm lại cầu nối bất cứ lúc nào.",
  "sources.addGoogleAccount": "Thêm tài khoản Google",
  "sources.addGoogleAccountHint":
    "Thêm một cầu nối Apps Script trong Cài đặt để liên kết thêm tài khoản Google",
  "sources.statusConnected": "Đã kết nối",
  "sources.statusPlaceholder": "Chỗ giữ chỗ",
  "sources.statusError": "Lỗi",
  "sources.genericConnected": "Khả dụng cho agent thông qua quy tắc quyền",
  "sources.genericPlaceholder": "Connector đã dựng khung; xác thực chưa được kết nối",

  // Workbench
  "workbench.title": "Workbench",
  "workbench.addMenu": "Thêm vào Workbench",
  "workbench.newFolder": "Thư mục mới",
  "workbench.addSpreadsheet": "Thêm bảng tính",
  "workbench.add": "Thêm",
  "workbench.create": "Tạo",
  "workbench.searchPlaceholder": "Tìm bảng tính",
  "workbench.ungrouped": "Chưa phân nhóm",
  "workbench.rename": "Đổi tên",
  "workbench.renameFolder": "Đổi tên thư mục",
  "workbench.delete": "Xóa",
  "workbench.remove": "Gỡ",
  "workbench.moveToFolder": "Chuyển vào thư mục",
  "workbench.folderMenu": "Thao tác thư mục {name}",
  "workbench.itemMenu": "Thao tác {name}",
  "workbench.emptyFolder": "Chưa có bảng tính",
  "workbench.emptyTitle": "Chưa có bảng tính nào",
  "workbench.emptyDescription":
    "Thêm một bảng tính Google Sheets để bắt đầu xây dựng không gian làm việc của bạn.",
  "workbench.noResults": "Không có kết quả",
  "workbench.folderNameLabel": "Tên thư mục",
  "workbench.folderNamePlaceholder": "ví dụ: Game Config",
  "workbench.addSpreadsheetDescription":
    "Dán liên kết Google Sheets hoặc id bảng tính, sau đó chọn thư mục.",
  "workbench.spreadsheetUrlLabel": "URL hoặc ID bảng tính",
  "workbench.spreadsheetUrlPlaceholder": "https://docs.google.com/spreadsheets/d/...",
  "workbench.folderLabel": "Thư mục",
  "workbench.deleteFolderTitle": "Xóa thư mục?",
  "workbench.deleteFolderDescription":
    "\"{name}\" sẽ bị xóa và các bảng tính của nó chuyển sang Chưa phân nhóm. Bản thân các bảng tính không bị gỡ.",
  "workbench.removeItemTitle": "Gỡ bảng tính?",
  "workbench.removeItemDescription":
    "\"{name}\" sẽ được gỡ khỏi Workbench. Bản thân bảng tính không bị xóa.",
  "workbench.findInSheet": "Tìm trong trang tính",
  "workbench.refresh": "Làm mới",
  "workbench.addRow": "Thêm hàng",
  "workbench.undo": "Hoàn tác",
  "workbench.redo": "Làm lại",
  "workbench.sheetTabs": "Thẻ trang tính",
  "workbench.selectPromptTitle": "Chọn hoặc thêm một bảng tính",
  "workbench.selectPromptDescription":
    "Chọn một bảng tính ở bên trái, hoặc thêm một bảng tính để bắt đầu.",
  "workbench.sheetLoadError": "Không thể tải trang tính này.",

  // Changes
  "changes.filterAll": "Tất cả",
  "changes.filterPending": "Đang chờ",
  "changes.filterApproved": "Đã duyệt",
  "changes.filterCommitted": "Đã commit",
  "changes.filterRejected": "Đã hủy bỏ",
  "changes.filterAria": "Lọc thay đổi theo trạng thái",
  "changes.emptyAll": "Chưa có thay đổi",
  "changes.emptyFiltered": "Không có thay đổi {filter}",
  "changes.emptyDescription": "Thao tác ghi của agent được lưu tại đây kèm phần khác biệt đã thay đổi",
  "changes.stagedDryRun": "Bản chạy thử · chưa được commit",
  "changes.discard": "Hủy bỏ",
  "changes.discarding": "Đang hủy bỏ...",
  "changes.statusPending": "Đang chờ",
  "changes.statusApproved": "Đã duyệt",
  "changes.statusCommitted": "Đã commit",
  "changes.statusRejected": "Đã hủy bỏ",
  "changes.committedBy": "Đã commit {time} bởi {who}",
  "changes.committed": "Đã commit {time}",
  "changes.rejected": "Đã hủy bỏ {time}",
  "changes.approvedWaiting": "Đã duyệt {time} · đang chờ agent commit",
  "changes.recordLabel": "Bản ghi {id}",
  "changes.formatCellsHeading": "Định dạng ô",
  "changes.formatLayoutHeading": "Bố cục sheet",
  "changes.formatFreezeRows": "Cố định {count} hàng đầu",
  "changes.formatFreezeColumns": "Cố định {count} cột đầu",
  "changes.formatColumnWidth": "Cột {column}: {pixels}px",

  // Records table
  "records.record": "Bản ghi",
  "records.previous": "Trước",
  "records.next": "Sau",
  "records.range": "{first}-{last} trên {total} bản ghi",

  // Settings - Appearance
  "settings.appearance.title": "Giao diện",
  "settings.appearance.theme": "Giao diện",
  "settings.appearance.themeFollowsSystem": "Theo tùy chọn hệ thống của bạn (hiện tại là {mode})",
  "settings.appearance.themeFixed": "Cố định cho thiết bị này",
  "settings.appearance.fontSize": "Cỡ chữ",
  "settings.appearance.fontSizeDescription": "Phóng to hoặc thu nhỏ toàn bộ giao diện.",
  "settings.appearance.fontSizeSmall": "Nhỏ",
  "settings.appearance.fontSizeNormal": "Vừa",
  "settings.appearance.fontSizeLarge": "Lớn",
  "settings.appearance.font": "Phông chữ",
  "settings.appearance.fontDescription":
    "Classic là phông serif, Modern là Inter, System dùng phông giao diện của hệ điều hành.",
  "settings.appearance.fontClassic": "Classic",
  "settings.appearance.fontModern": "Modern",
  "settings.appearance.fontSystem": "Hệ thống",
  "settings.appearance.language": "Ngôn ngữ",
  "settings.appearance.languageDescription": "Chọn ngôn ngữ cho giao diện.",
  "settings.appearance.languageEnglish": "Tiếng Anh",
  "settings.appearance.languageVietnamese": "Tiếng Việt",
  "settings.bridges.title": "Cầu nối Google",
  "settings.bridges.description":
    "Mỗi cầu nối là một web app Apps Script nhỏ được triển khai trên một tài khoản Google. Nó cấp token truy cập ngắn hạn, nên không cần dự án Cloud Console hay OAuth client.",
  "settings.bridges.empty":
    "Chưa có cầu nối nào. Thêm một cầu nối bên dưới để kết nối tài khoản Google.",
  "settings.bridges.deployment": "Deployment",
  "settings.bridges.missingCredential":
    "Tài khoản này chưa lưu cầu nối nào. Hãy xóa nó rồi thêm lại cầu nối.",
  "settings.bridges.test": "Kiểm tra",
  "settings.bridges.testing": "Đang kiểm tra...",
  "settings.bridges.remove": "Xóa",
  "settings.bridges.removeTitle": "Xóa cầu nối?",
  "settings.bridges.removeDescription":
    "Agent mất quyền truy cập vào bảng tính của {email}, và URL cùng secret của cầu nối sẽ bị xóa khỏi keychain của hệ điều hành. Bản triển khai Apps Script không bị thay đổi.",
  "settings.bridges.addTitle": "Thêm cầu nối",
  "settings.bridges.url": "URL web app",
  "settings.bridges.secret": "Secret",
  "settings.bridges.secretPlaceholder": "Giá trị được setup() ghi vào log",
  "settings.bridges.secretHint":
    "Được lưu trong keychain của hệ điều hành cùng với URL; nó chỉ được gửi tới cầu nối của bạn.",
  "settings.bridges.add": "Thêm cầu nối",
  "settings.bridges.adding": "Đang thêm...",
  "settings.bridges.guideToggle": "Cách tạo cầu nối",
  "settings.bridges.step1":
    "Mở script.google.com bằng tài khoản Google mà agent sẽ dùng và tạo một dự án mới.",
  "settings.bridges.step2":
    "Trong Project Settings, đánh dấu \"Show appsscript.json manifest file in editor\".",
  "settings.bridges.step3": "Thay nội dung của Code.gs và appsscript.json bằng hai tệp bên dưới.",
  "settings.bridges.step4":
    "Chọn hàm setup và bấm Run, cấp các quyền được yêu cầu, rồi sao chép giá trị SECRET từ execution log.",
  "settings.bridges.step5":
    "Bấm Deploy > New deployment, chọn Web app với Execute as: Me và Who has access: Anyone, rồi sao chép URL web app kết thúc bằng /exec.",
  "settings.bridges.step6": "Dán URL và secret vào phía trên rồi bấm Thêm cầu nối.",
  "settings.bridges.copyCode": "Sao chép Code.gs",
  "settings.bridges.copyManifest": "Sao chép appsscript.json",

  // Settings - Google Sheets

  // Settings - Google JSON import modal

  // Settings - MCP Server
  "settings.mcpServer.title": "Máy chủ MCP",
  "settings.mcpServer.transport": "Giao thức truyền",
  "settings.mcpServer.transportDescription":
    "Stdio khởi chạy sidecar cho mỗi client; Local HTTP phục vụ một endpoint dùng chung. Sau khi cập nhật app, Local HTTP tự nhận tool mới, còn client stdio phải khởi động lại.",
  "settings.mcpServer.transportAria": "Giao thức truyền MCP",
  "settings.mcpServer.transportStdio": "Stdio",
  "settings.mcpServer.transportHttp": "HTTP cục bộ",
  "settings.mcpServer.httpPort": "Cổng HTTP",
  "settings.mcpServer.httpPortHint": "Cổng loopback cho endpoint HTTP cục bộ. Khoảng {min}-{max}.",
  "settings.mcpServer.enterPort": "Nhập cổng",
  "settings.mcpServer.portWholeNumber": "Cổng phải là số nguyên",
  "settings.mcpServer.portRange": "Cổng phải nằm trong khoảng {min} đến {max}",
  "settings.mcpServer.serverProcess": "Tiến trình máy chủ",
  "settings.mcpServer.serverProcessHttp":
    "Chạy endpoint HTTP dùng chung như một tiến trình do ứng dụng quản lý.",
  "settings.mcpServer.serverProcessStdio":
    "Client MCP thường tự khởi chạy phiên bản riêng; cái này chạy một phiên bản cục bộ được quản lý.",
  "settings.mcpServer.start": "Bắt đầu",
  "settings.mcpServer.starting": "Đang bắt đầu...",
  "settings.mcpServer.stop": "Dừng",
  "settings.mcpServer.stopping": "Đang dừng...",
  "settings.mcpServer.endpointUrl": "URL endpoint",
  "settings.mcpServer.copyEndpoint": "Sao chép URL endpoint",
  "settings.mcpServer.restartHint":
    "Thay đổi giao thức truyền hoặc cổng cần khởi động lại sidecar để có hiệu lực.",
  "settings.mcpServer.configuring": "Đang cấu hình...",

  // Settings - MCP Clients
  "settings.mcpClients.title": "Client MCP",
  "settings.mcpClients.client": "Client",
  "settings.mcpClients.selectClient": "Chọn một client",
  "settings.mcpClients.clientAria": "Client MCP",
  "settings.mcpClients.noneDetected": "Không phát hiện client MCP được hỗ trợ",
  "settings.mcpClients.configure": "Cấu hình",
  "settings.mcpClients.configuring": "Đang cấu hình...",
  "settings.mcpClients.unregister": "Hủy đăng ký",
  "settings.mcpClients.configureAll": "Cấu hình tất cả client đã phát hiện",
  "settings.mcpClients.configFile": "Tệp cấu hình",
  "settings.mcpClients.notInstalled": "{name} chưa được cài đặt",
  "settings.mcpClients.alreadyConfigured": "Đã cấu hình",
  "settings.mcpClients.noneNeedConfigure": "Không có client đã phát hiện nào cần cấu hình",
  "settings.mcpClients.stateConfigured": "Đã cấu hình",
  "settings.mcpClients.stateMissingConfig": "Thiếu cấu hình",
  "settings.mcpClients.stateNotFound": "Không tìm thấy",
  "settings.mcpClients.stateUnknown": "Không xác định",
  "settings.mcpClients.unregisterTitle": "Hủy đăng ký khỏi {name}?",
  "settings.mcpClients.unregisterDescription":
    "Thao tác này chỉnh sửa tệp cấu hình của {name} để xóa máy chủ MCP Sheet Port. Bạn có thể cấu hình lại bất cứ lúc nào.",

  // Settings - Permissions
  "settings.permissions.title": "Quyền",
  "settings.permissions.connectFirst": "Kết nối một nguồn dữ liệu trước",
  "settings.permissions.hint":
    "Chọn một preset quyền truy cập cho mỗi nguồn. Thao tác ghi được phép của agent áp dụng ngay và được lưu trong Thay đổi.",
  "settings.permissions.presetAria": "Preset quyền cho {name}",
  "settings.permissions.custom": "Tùy chỉnh",
  "settings.permissions.customHint":
    "Nguồn này dùng quy tắc tùy chỉnh. Chọn một preset để chuẩn hóa.",
  "settings.permissions.updated": "Cập nhật {time}",
  "settings.permissions.updatedPrefix": "Cập nhật",
  "settings.permissions.bypassTitle": "Bỏ qua quyền?",
  "settings.permissions.bypassDescription":
    "Agent có toàn quyền truy cập nguồn này, bao gồm cả xóa. Chỉ chọn nếu bạn hoàn toàn tin tưởng mọi agent đã kết nối.",
  "settings.permissions.enableBypass": "Bật Bypass",

  // Permission presets
  "preset.readOnly.label": "Chỉ đọc",
  "preset.readOnly.description":
    "Agent có thể đọc bản ghi nhưng không thể ghi, cập nhật hoặc xóa.",
  "preset.readWrite.label": "Đọc & ghi",
  "preset.readWrite.description":
    "Agent có thể đọc, thêm và cập nhật bản ghi. Thao tác xóa vẫn bị chặn.",
  "preset.bypass.label": "Bỏ qua quyền",
  "preset.bypass.description":
    "Toàn quyền truy cập, bao gồm cả xóa.",

  // Settings - About
  "settings.about.title": "Giới thiệu",
  "settings.about.checkUpdates": "Kiểm tra cập nhật",
  "settings.about.checking": "Đang kiểm tra...",
  "settings.about.appName": "Tên ứng dụng",
  "settings.about.version": "Phiên bản",
  "settings.about.createdBy": "Tạo bởi",
  "settings.about.database": "Cơ sở dữ liệu",
  "settings.about.upToDate": "Bạn đang dùng phiên bản mới nhất",
  "settings.about.updateCheckFailed": "Kiểm tra cập nhật thất bại",
  "settings.about.updateAvailableTitle": "Có bản cập nhật",
  "settings.about.updateAvailableVersion":
    "Phiên bản {version} sẽ được tải xuống và cài đặt. Ứng dụng sẽ khởi động lại để hoàn tất.",
  "settings.about.updateAvailableGeneric":
    "Một phiên bản mới hơn sẽ được tải xuống và cài đặt. Ứng dụng sẽ khởi động lại để hoàn tất.",
  "settings.about.releaseNotes": "Ghi chú phát hành",
  "settings.about.install": "Cài đặt",
  "settings.about.installing": "Đang cài đặt...",

  // Settings - General
  "settings.general.title": "Chung",
  "settings.general.whenClosing": "Khi đóng cửa sổ",
  "settings.general.whenClosingDescription":
    "Hỏi mỗi lần, tiếp tục chạy trong khay hệ thống, hoặc thoát ứng dụng.",
  "settings.general.closeAsk": "Hỏi",
  "settings.general.closeTray": "Chạy nền",
  "settings.general.closeQuit": "Thoát",
  "settings.general.launchAtLogin": "Khởi động khi đăng nhập",
  "settings.general.launchAtLoginDescription":
    "Tự động khởi động ứng dụng khi bạn đăng nhập.",

  // Settings - Reset
  "settings.reset.title": "Đặt lại",
  "settings.reset.description":
    "Khôi phục tùy chọn về mặc định. Cầu nối Google, quy tắc quyền và dữ liệu của bạn không bị ảnh hưởng.",
  "settings.reset.button": "Đặt lại mặc định",
  "settings.reset.confirmTitle": "Đặt lại mặc định?",
  "settings.reset.confirmDescription":
    "Giao diện, phông chữ và ngôn ngữ trở về mặc định. Thao tác này KHÔNG xóa cầu nối Google, quy tắc quyền hoặc dữ liệu của bạn.",

  // Close behavior dialog
  "closeDialog.title": "Chạy nền?",
  "closeDialog.description": "Tiếp tục chạy ứng dụng trong khay hệ thống, hoặc thoát hẳn.",
  "closeDialog.rememberChoice": "Ghi nhớ lựa chọn của tôi",
  "closeDialog.quit": "Thoát",
  "closeDialog.quitting": "Đang thoát...",
  "closeDialog.runInBackground": "Chạy nền",
  "closeDialog.minimizing": "Đang thu nhỏ...",

  // Sidebar update card
  "sidebar.updateAvailable": "Có bản cập nhật",
  "sidebar.updateAvailableVersion": "Có bản cập nhật: v{version}",
  "sidebar.downloadingUpdate": "Đang tải bản cập nhật...",
  "sidebar.downloading": "Đang tải...",
  "sidebar.update": "Cập nhật",

  // Empty states
  "empty.records.title": "Không có bản ghi",
  "empty.records.description":
    "Bảng này trống. Các bản ghi agent thêm sẽ hiển thị tại đây sau khi commit",

  // Toasts - settings
  "toast.updatedRestartClients":
    "Đã cập nhật lên v{version}. Khởi động lại MCP client (Claude Desktop, Claude Code, ...) để nạp tool mới.",
  "toast.fontSizeError": "Không cập nhật được cỡ chữ",
  "toast.fontError": "Không cập nhật được phông chữ",
  "toast.languageError": "Không cập nhật được ngôn ngữ",
  "toast.languageUpdated": "Đã cập nhật ngôn ngữ",
  "toast.resetFailed": "Đặt lại thất bại",
  "toast.settingsReset": "Đã đặt lại cài đặt về mặc định",
  "toast.closeBehaviorError": "Không cập nhật được hành vi đóng",
  "toast.launchAtLoginError": "Không cập nhật được khởi động khi đăng nhập",
  "toast.launchAtLoginEnabled": "Đã bật khởi động khi đăng nhập",
  "toast.launchAtLoginDisabled": "Đã tắt khởi động khi đăng nhập",
  "toast.bridgeAdded": "Đã thêm cầu nối Google",
  "toast.bridgeAddError": "Chưa thêm được cầu nối",
  "toast.bridgeSignedInAs": "Đăng nhập với {email}",
  "toast.bridgeTestOk": "Cầu nối hoạt động bình thường",
  "toast.bridgeTestError": "Kiểm tra cầu nối thất bại",
  "toast.bridgeRemoved": "Đã xóa cầu nối Google",
  "toast.bridgeRemoveError": "Chưa xóa được cầu nối",

  // Toasts - Google

  // Toasts - MCP
  "toast.transportError": "Không cập nhật được giao thức truyền",
  "toast.transportSaved": "Đã lưu giao thức truyền MCP",
  "toast.restartToApply": "Khởi động lại sidecar để áp dụng",
  "toast.portError": "Không lưu được cổng",
  "toast.portSaved": "Đã lưu cổng MCP",
  "toast.clientConfigError": "Không cấu hình được client",
  "toast.clientConfigured": "Đã cấu hình client MCP",
  "toast.clientUnregisterError": "Không hủy đăng ký được client",
  "toast.clientUnregistered": "Đã hủy đăng ký client MCP",
  "toast.serverStartError": "Không khởi động được máy chủ MCP",
  "toast.serverStarted": "Đã khởi động máy chủ MCP",
  "toast.serverStopError": "Không dừng được máy chủ MCP",
  "toast.serverStopped": "Đã dừng máy chủ MCP",
  "toast.clientsConfigError": "Không cấu hình được các client",
  "toast.clientsConfigured": "Đã cấu hình các client MCP đã phát hiện",
  "toast.changeDiscarded": "Đã hủy bỏ thay đổi",
  "toast.changeDiscardError": "Chưa hủy bỏ được thay đổi",

  // Toasts - Changes

  // Toasts - Workbench
  "toast.folderCreated": "Đã tạo thư mục",
  "toast.folderCreateError": "Không tạo được thư mục",
  "toast.folderRenamed": "Đã đổi tên thư mục",
  "toast.folderRenameError": "Không đổi tên được thư mục",
  "toast.folderDeleted": "Đã xóa thư mục",
  "toast.folderDeleteError": "Không xóa được thư mục",
  "toast.spreadsheetAdded": "Đã thêm bảng tính",
  "toast.spreadsheetAddError": "Không thêm được bảng tính",
  "toast.spreadsheetRemoved": "Đã gỡ bảng tính",
  "toast.spreadsheetRemoveError": "Không gỡ được bảng tính",
  "toast.spreadsheetMoved": "Đã chuyển bảng tính",
  "toast.spreadsheetMoveError": "Không chuyển được bảng tính",
  "toast.cellUpdateError": "Không cập nhật được ô",
  "toast.rowAdded": "Đã thêm hàng",
  "toast.rowAddError": "Không thêm được hàng"
};

export const translations: Record<Language, Dictionary> = { en, vi };
