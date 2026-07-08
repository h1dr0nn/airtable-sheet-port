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
    "Every agent write lands here as a preview before it can commit",
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
  "palette.connectGoogleSheets": "Connect Google Sheets",

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
  "dashboard.pendingApprovals": "Pending Approvals",
  "dashboard.nothingWaiting": "Nothing waiting on you",
  "dashboard.oneChangeAwaiting": "Change awaiting review",
  "dashboard.changesAwaiting": "Changes awaiting review",
  "dashboard.reviewChanges": "Review Changes",
  "dashboard.database": "Database",
  "dashboard.copyDatabasePath": "Copy database path",
  "dashboard.sharedSqlite": "Shared SQLite",
  "dashboard.version": "Version",
  "dashboard.tokenVault": "Token Vault",
  "dashboard.googleSheets": "Google Sheets",
  "dashboard.provider": "Provider",
  "dashboard.inKeychain": "In Keychain",
  "dashboard.notStored": "Not Stored",
  "dashboard.tokensNeverLeave": "Tokens never leave the OS keychain.",
  "dashboard.noSourcesTitle": "No Data Sources Connected",
  "dashboard.noSourcesDescription":
    "Connect a data source such as Google Sheets to give agents something to read.",
  "dashboard.connectDataSource": "Connect a Data Source",
  "dashboard.recentActivity": "Recent Activity",
  "dashboard.recentActivityEmpty": "Agent activity shows up here as it happens.",
  "dashboard.recentChanges": "Recent Changes",
  "dashboard.viewAll": "View All",
  "dashboard.recentChangesEmpty": "Agent write previews land here for review.",

  // Data Sources
  "sources.googleSheets": "Google Sheets",
  "sources.disconnect": "Disconnect",
  "sources.disconnectTooltip": "Remove this account and its stored token",
  "sources.linkedTo": "Linked to {email}",
  "sources.disconnectTitle": "Disconnect Google Account?",
  "sources.disconnectDescription":
    "Agents lose access to this account's spreadsheets and the stored token is removed from the OS keychain. You can reconnect at any time.",
  "sources.addGoogleAccount": "Add Google Account",
  "sources.connecting": "Connecting...",
  "sources.addGoogleAccountHint":
    "Link another Google account so agents can reach more spreadsheets",
  "sources.finishSignIn": "Finish signing in with Google in your browser",
  "sources.saveSecretFirst": "Save the OAuth client secret in Settings first",
  "sources.setClientIdFirst": "Set the OAuth client ID in Settings first",
  "sources.configureGoogle": "Configure Google in Settings",
  "sources.additionalProvider": "Additional Provider",
  "sources.additionalProviderHint":
    "A second table provider lands here once its connector ships",
  "sources.comingSoon": "Coming Soon",
  "sources.connect": "Connect",
  "sources.connectTooltip": "Available once the connector ships",
  "sources.notAvailableYet": "Not available yet",
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
  "changes.filterRejected": "Rejected",
  "changes.filterAria": "Filter changes by status",
  "changes.emptyAll": "No Changes Yet",
  "changes.emptyFiltered": "No {filter} Changes",
  "changes.emptyDescription": "When an agent previews a write it appears here for review",
  "changes.needsConfirmation": "Needs Confirmation",
  "changes.needsConfirmationTooltip": "Policy requires user confirmation before commit",
  "changes.awaitingDecision": "Awaiting your decision",
  "changes.reject": "Reject",
  "changes.rejecting": "Rejecting...",
  "changes.approve": "Approve",
  "changes.approving": "Approving...",
  "changes.statusPending": "Pending",
  "changes.statusApproved": "Approved",
  "changes.statusCommitted": "Committed",
  "changes.statusRejected": "Rejected",
  "changes.committedBy": "Committed {time} by {who}",
  "changes.committed": "Committed {time}",
  "changes.rejected": "Rejected {time}",
  "changes.approvedWaiting": "Approved {time} · waiting for the agent to commit",
  "changes.autoCommit": "Auto-commit · no confirmation required by policy",
  "changes.recordLabel": "Record {id}",

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

  // Settings - Google Sheets
  "settings.google.title": "Google Sheets",
  "settings.google.importJson": "Import JSON",
  "settings.google.importing": "Importing...",
  "settings.google.clientId": "OAuth Client ID",
  "settings.google.clientIdHint": "Desktop-app client ID from Google Cloud Console.",
  "settings.google.clientSecret": "OAuth Client Secret",
  "settings.google.clientSecretHint":
    "Google requires the Desktop-app client secret when exchanging the sign-in code; it never leaves the keychain.",
  "settings.google.storedInKeychain": "•••••••• Stored in OS keychain",
  "settings.google.replace": "Replace",
  "settings.google.clear": "Clear",
  "settings.google.clearSecretTitle": "Clear Client Secret?",
  "settings.google.clearSecretDescription":
    "The client secret is removed from the OS keychain. Google sign-in will fail until a new secret is saved.",
  "settings.google.accountsLinked":
    "{count} account(s) linked. Manage them in Data Sources.",
  "settings.google.connectFromSources": "Connect an account from the Data Sources screen",

  // Settings - Google JSON import modal
  "settings.import.invalidTitle": "Invalid Credentials File",
  "settings.import.invalidDescription":
    "The selected file could not be used. Fix the following and try again.",
  "settings.import.successTitle": "Credentials Imported",
  "settings.import.successDescription": "Client ID and secret saved to the OS keychain",
  "settings.import.failed": "Import failed",

  // Settings - MCP Server
  "settings.mcpServer.title": "MCP Server",
  "settings.mcpServer.transport": "Transport",
  "settings.mcpServer.transportDescription":
    "Stdio spawns the sidecar per client; Local HTTP serves one shared endpoint.",
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
    "Pick an access preset per source. Auto Approve and Bypass turn on global auto-approve, which applies to every connected source.",
  "settings.permissions.presetAria": "Permission preset for {name}",
  "settings.permissions.custom": "Custom",
  "settings.permissions.customHint":
    "This source uses a custom rule. Pick a preset to normalize it.",
  "settings.permissions.updated": "Updated {time}",
  "settings.permissions.updatedPrefix": "Updated",
  "settings.permissions.bypassTitle": "Bypass Permission?",
  "settings.permissions.bypassDescription":
    "Agents get full access including deletes, with no approval gate, and global auto-approve is turned on. Only choose this if you fully trust every connected agent.",
  "settings.permissions.enableBypass": "Enable Bypass",

  // Permission presets (lib/permissionPresets.ts)
  "preset.readOnly.label": "Read Only",
  "preset.readOnly.description":
    "Agents can read records but cannot write, update, or delete.",
  "preset.ask.label": "Ask Permissions",
  "preset.ask.description":
    "Agents can write, but appends, updates, and deletes wait for your approval.",
  "preset.autoApprove.label": "Auto Approve",
  "preset.autoApprove.description":
    "Agents write without asking. Deletes stay blocked. Enables global auto-approve.",
  "preset.bypass.label": "Bypass Permission",
  "preset.bypass.description":
    "Full access including deletes, with no approval gate. Enables global auto-approve.",

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
    "Restore preferences to their defaults. Your Google credentials, permission rules, and data are not affected.",
  "settings.reset.button": "Reset to Default",
  "settings.reset.confirmTitle": "Reset to Default?",
  "settings.reset.confirmDescription":
    "Theme, font, and auto-approve return to their defaults. This does NOT remove your Google credentials, permission rules, or data.",

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
  "toast.autoApproveError": "Auto-approve not updated",
  "toast.autoApproveEnabled": "Auto-approve enabled",
  "toast.autoApproveDisabled": "Auto-approve disabled",
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

  // Toasts - Google
  "toast.clientIdError": "Client ID not saved",
  "toast.clientIdSaved": "Google client ID saved",
  "toast.clientSecretError": "Client secret not saved",
  "toast.clientSecretCleared": "Google client secret cleared",
  "toast.clientSecretSaved": "Google client secret saved",
  "toast.clientSecretSavedDesc": "Stored in the OS keychain",
  "toast.googleConnectError": "Google Sheets connection failed",
  "toast.googleConnected": "Google Sheets connected",
  "toast.googleConnectedDesc": "Signed in as {email}",
  "toast.googleDisconnectError": "Google Sheets disconnect failed",
  "toast.googleDisconnected": "Google Sheets disconnected",

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

  // Toasts - Changes
  "toast.changeDecisionFailed": "Change decision failed",
  "toast.changeApproved": "Change approved",
  "toast.changeRejected": "Change rejected",

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
    "Mọi thao tác ghi của agent đến đây dưới dạng bản xem trước trước khi được commit",
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
  "palette.connectGoogleSheets": "Kết nối Google Sheets",

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
  "dashboard.pendingApprovals": "Chờ phê duyệt",
  "dashboard.nothingWaiting": "Không có gì chờ bạn",
  "dashboard.oneChangeAwaiting": "Thay đổi đang chờ xem xét",
  "dashboard.changesAwaiting": "Thay đổi đang chờ xem xét",
  "dashboard.reviewChanges": "Xem xét thay đổi",
  "dashboard.database": "Cơ sở dữ liệu",
  "dashboard.copyDatabasePath": "Sao chép đường dẫn cơ sở dữ liệu",
  "dashboard.sharedSqlite": "SQLite dùng chung",
  "dashboard.version": "Phiên bản",
  "dashboard.tokenVault": "Kho token",
  "dashboard.googleSheets": "Google Sheets",
  "dashboard.provider": "Nhà cung cấp",
  "dashboard.inKeychain": "Trong keychain",
  "dashboard.notStored": "Chưa lưu",
  "dashboard.tokensNeverLeave": "Token không bao giờ rời khỏi keychain của hệ điều hành.",
  "dashboard.noSourcesTitle": "Chưa kết nối nguồn dữ liệu",
  "dashboard.noSourcesDescription":
    "Kết nối một nguồn dữ liệu như Google Sheets để agent có thể đọc.",
  "dashboard.connectDataSource": "Kết nối nguồn dữ liệu",
  "dashboard.recentActivity": "Hoạt động gần đây",
  "dashboard.recentActivityEmpty": "Hoạt động của agent hiển thị tại đây khi xảy ra.",
  "dashboard.recentChanges": "Thay đổi gần đây",
  "dashboard.viewAll": "Xem tất cả",
  "dashboard.recentChangesEmpty": "Bản xem trước thao tác ghi của agent xuất hiện tại đây để xem xét.",

  // Data Sources
  "sources.googleSheets": "Google Sheets",
  "sources.disconnect": "Ngắt kết nối",
  "sources.disconnectTooltip": "Xóa tài khoản này và token đã lưu",
  "sources.linkedTo": "Liên kết với {email}",
  "sources.disconnectTitle": "Ngắt kết nối tài khoản Google?",
  "sources.disconnectDescription":
    "Agent mất quyền truy cập vào bảng tính của tài khoản này và token đã lưu sẽ bị xóa khỏi keychain của hệ điều hành. Bạn có thể kết nối lại bất cứ lúc nào.",
  "sources.addGoogleAccount": "Thêm tài khoản Google",
  "sources.connecting": "Đang kết nối...",
  "sources.addGoogleAccountHint":
    "Liên kết thêm tài khoản Google để agent truy cập được nhiều bảng tính hơn",
  "sources.finishSignIn": "Hoàn tất đăng nhập Google trong trình duyệt của bạn",
  "sources.saveSecretFirst": "Lưu client secret OAuth trong Cài đặt trước",
  "sources.setClientIdFirst": "Đặt client ID OAuth trong Cài đặt trước",
  "sources.configureGoogle": "Cấu hình Google trong Cài đặt",
  "sources.additionalProvider": "Nhà cung cấp bổ sung",
  "sources.additionalProviderHint":
    "Một nhà cung cấp bảng thứ hai sẽ xuất hiện tại đây khi connector sẵn sàng",
  "sources.comingSoon": "Sắp ra mắt",
  "sources.connect": "Kết nối",
  "sources.connectTooltip": "Có sẵn khi connector ra mắt",
  "sources.notAvailableYet": "Chưa khả dụng",
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
  "changes.filterRejected": "Đã từ chối",
  "changes.filterAria": "Lọc thay đổi theo trạng thái",
  "changes.emptyAll": "Chưa có thay đổi",
  "changes.emptyFiltered": "Không có thay đổi {filter}",
  "changes.emptyDescription": "Khi agent xem trước một thao tác ghi, nó sẽ xuất hiện tại đây để xem xét",
  "changes.needsConfirmation": "Cần xác nhận",
  "changes.needsConfirmationTooltip": "Chính sách yêu cầu người dùng xác nhận trước khi commit",
  "changes.awaitingDecision": "Đang chờ bạn quyết định",
  "changes.reject": "Từ chối",
  "changes.rejecting": "Đang từ chối...",
  "changes.approve": "Phê duyệt",
  "changes.approving": "Đang phê duyệt...",
  "changes.statusPending": "Đang chờ",
  "changes.statusApproved": "Đã duyệt",
  "changes.statusCommitted": "Đã commit",
  "changes.statusRejected": "Đã từ chối",
  "changes.committedBy": "Đã commit {time} bởi {who}",
  "changes.committed": "Đã commit {time}",
  "changes.rejected": "Đã từ chối {time}",
  "changes.approvedWaiting": "Đã duyệt {time} · đang chờ agent commit",
  "changes.autoCommit": "Tự động commit · chính sách không yêu cầu xác nhận",
  "changes.recordLabel": "Bản ghi {id}",

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

  // Settings - Google Sheets
  "settings.google.title": "Google Sheets",
  "settings.google.importJson": "Nhập JSON",
  "settings.google.importing": "Đang nhập...",
  "settings.google.clientId": "OAuth Client ID",
  "settings.google.clientIdHint": "Client ID ứng dụng máy tính từ Google Cloud Console.",
  "settings.google.clientSecret": "OAuth Client Secret",
  "settings.google.clientSecretHint":
    "Google yêu cầu client secret ứng dụng máy tính khi trao đổi mã đăng nhập; nó không bao giờ rời khỏi keychain.",
  "settings.google.storedInKeychain": "•••••••• Đã lưu trong keychain của hệ điều hành",
  "settings.google.replace": "Thay thế",
  "settings.google.clear": "Xóa",
  "settings.google.clearSecretTitle": "Xóa Client Secret?",
  "settings.google.clearSecretDescription":
    "Client secret sẽ bị xóa khỏi keychain của hệ điều hành. Đăng nhập Google sẽ thất bại cho đến khi lưu secret mới.",
  "settings.google.accountsLinked": "Đã liên kết {count} tài khoản. Quản lý chúng trong Nguồn dữ liệu.",
  "settings.google.connectFromSources": "Kết nối tài khoản từ màn hình Nguồn dữ liệu",

  // Settings - Google JSON import modal
  "settings.import.invalidTitle": "Tệp thông tin xác thực không hợp lệ",
  "settings.import.invalidDescription":
    "Không thể sử dụng tệp đã chọn. Khắc phục các vấn đề sau rồi thử lại.",
  "settings.import.successTitle": "Đã nhập thông tin xác thực",
  "settings.import.successDescription": "Đã lưu Client ID và secret vào keychain của hệ điều hành",
  "settings.import.failed": "Nhập thất bại",

  // Settings - MCP Server
  "settings.mcpServer.title": "Máy chủ MCP",
  "settings.mcpServer.transport": "Giao thức truyền",
  "settings.mcpServer.transportDescription":
    "Stdio khởi chạy sidecar cho mỗi client; Local HTTP phục vụ một endpoint dùng chung.",
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
    "Chọn một preset quyền truy cập cho mỗi nguồn. Auto Approve và Bypass bật auto-approve toàn cục, áp dụng cho mọi nguồn đã kết nối.",
  "settings.permissions.presetAria": "Preset quyền cho {name}",
  "settings.permissions.custom": "Tùy chỉnh",
  "settings.permissions.customHint":
    "Nguồn này dùng quy tắc tùy chỉnh. Chọn một preset để chuẩn hóa.",
  "settings.permissions.updated": "Cập nhật {time}",
  "settings.permissions.updatedPrefix": "Cập nhật",
  "settings.permissions.bypassTitle": "Bỏ qua quyền?",
  "settings.permissions.bypassDescription":
    "Agent có toàn quyền truy cập bao gồm cả xóa, không có cổng phê duyệt, và auto-approve toàn cục được bật. Chỉ chọn nếu bạn hoàn toàn tin tưởng mọi agent đã kết nối.",
  "settings.permissions.enableBypass": "Bật Bypass",

  // Permission presets
  "preset.readOnly.label": "Chỉ đọc",
  "preset.readOnly.description":
    "Agent có thể đọc bản ghi nhưng không thể ghi, cập nhật hoặc xóa.",
  "preset.ask.label": "Hỏi quyền",
  "preset.ask.description":
    "Agent có thể ghi, nhưng các thao tác thêm, cập nhật và xóa chờ bạn phê duyệt.",
  "preset.autoApprove.label": "Tự động duyệt",
  "preset.autoApprove.description":
    "Agent ghi mà không cần hỏi. Xóa vẫn bị chặn. Bật auto-approve toàn cục.",
  "preset.bypass.label": "Bỏ qua quyền",
  "preset.bypass.description":
    "Toàn quyền truy cập bao gồm cả xóa, không có cổng phê duyệt. Bật auto-approve toàn cục.",

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
    "Khôi phục tùy chọn về mặc định. Thông tin xác thực Google, quy tắc quyền và dữ liệu của bạn không bị ảnh hưởng.",
  "settings.reset.button": "Đặt lại mặc định",
  "settings.reset.confirmTitle": "Đặt lại mặc định?",
  "settings.reset.confirmDescription":
    "Giao diện, phông chữ và auto-approve trở về mặc định. Thao tác này KHÔNG xóa thông tin xác thực Google, quy tắc quyền hoặc dữ liệu của bạn.",

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
  "toast.autoApproveError": "Không cập nhật được auto-approve",
  "toast.autoApproveEnabled": "Đã bật auto-approve",
  "toast.autoApproveDisabled": "Đã tắt auto-approve",
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

  // Toasts - Google
  "toast.clientIdError": "Không lưu được Client ID",
  "toast.clientIdSaved": "Đã lưu Google client ID",
  "toast.clientSecretError": "Không lưu được client secret",
  "toast.clientSecretCleared": "Đã xóa Google client secret",
  "toast.clientSecretSaved": "Đã lưu Google client secret",
  "toast.clientSecretSavedDesc": "Đã lưu trong keychain của hệ điều hành",
  "toast.googleConnectError": "Kết nối Google Sheets thất bại",
  "toast.googleConnected": "Đã kết nối Google Sheets",
  "toast.googleConnectedDesc": "Đã đăng nhập với {email}",
  "toast.googleDisconnectError": "Ngắt kết nối Google Sheets thất bại",
  "toast.googleDisconnected": "Đã ngắt kết nối Google Sheets",

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

  // Toasts - Changes
  "toast.changeDecisionFailed": "Quyết định thay đổi thất bại",
  "toast.changeApproved": "Đã phê duyệt thay đổi",
  "toast.changeRejected": "Đã từ chối thay đổi",

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
