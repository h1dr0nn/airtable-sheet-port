import type { SidecarHeartbeat } from "./ipc.js";

/**
 * Outdated-sidecar detection for the Dashboard MCP card. After an app update,
 * stdio MCP clients (Claude Desktop, Claude Code, ...) keep running the OLD
 * sidecar binary until they restart, so any fresh heartbeat whose version is
 * missing (sidecar predates the heartbeat `version` column) or differs from
 * the app version is flagged.
 */

/**
 * The bare semver of a version string: drops a leading "v" and anything after
 * the first whitespace, so "v2.2.1" and "0.0.1 (browser demo)" compare as
 * "2.2.1" / "0.0.1".
 */
export function normalizeVersion(version: string): string {
  const [first = ""] = version.trim().split(/\s+/);
  return first.replace(/^v/i, "");
}

/** True when the sidecar reports no version or a different one than the app. */
export function isSidecarOutdated(sidecar: SidecarHeartbeat, appVersion: string): boolean {
  if (!sidecar.version) {
    return true;
  }
  return normalizeVersion(sidecar.version) !== normalizeVersion(appVersion);
}

export function outdatedSidecars(sidecars: SidecarHeartbeat[], appVersion: string): SidecarHeartbeat[] {
  return sidecars.filter((sidecar) => isSidecarOutdated(sidecar, appVersion));
}

/**
 * The single old version to name in the warning, or null when it cannot be
 * named precisely (an unversioned sidecar, or several different versions), in
 * which case the UI says "an older version".
 */
export function outdatedVersionLabel(outdated: SidecarHeartbeat[]): string | null {
  const versions = new Set<string>();
  for (const sidecar of outdated) {
    if (!sidecar.version) {
      return null;
    }
    versions.add(normalizeVersion(sidecar.version));
  }
  if (versions.size !== 1) {
    return null;
  }
  const [only] = versions;
  return only ?? null;
}

// ---------------------------------------------------------------------------
// Client labels. Each sidecar records the MCP client's `initialize`
// clientInfo (schema_version 6), plus its own and its parent's executable.
// ---------------------------------------------------------------------------

/** Friendly names for known `clientInfo.name` values (lowercased). */
const KNOWN_CLIENTS: Record<string, string> = {
  "claude-code": "Claude Code",
  // Claude Desktop sends "claude-ai"; the other spellings are defensive.
  "claude-ai": "Claude Desktop",
  "claude-desktop": "Claude Desktop",
  "claude desktop": "Claude Desktop",
  cursor: "Cursor",
  "cursor-vscode": "Cursor",
  windsurf: "Windsurf",
  "windsurf-client": "Windsurf",
  "visual studio code": "VS Code",
  "visual studio code - insiders": "VS Code Insiders",
  vscode: "VS Code",
  codex: "Codex",
  "codex-mcp-client": "Codex",
  "gemini-cli": "Gemini CLI",
  "gemini-cli-mcp-client": "Gemini CLI",
  zed: "Zed",
  cline: "Cline",
  continue: "Continue",
  "mcp-inspector": "MCP Inspector",
  "inspector-client": "MCP Inspector"
};

/** Prefix fallbacks for clients that append a variant to their name. */
const CLIENT_PREFIXES: [prefix: string, label: string][] = [
  ["cursor", "Cursor"],
  ["windsurf", "Windsurf"],
  ["visual studio code", "VS Code"],
  ["codex", "Codex"],
  ["gemini-cli", "Gemini CLI"]
];

export const CLAUDE_CODE_IN_DESKTOP_LABEL = "Claude Code (Claude Desktop)";

/** Lowercased path with forward slashes and no trailing slash. */
export function normalizePath(path: string): string {
  return path.trim().replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
}

function directoryOf(path: string): string {
  const normalized = normalizePath(path);
  const cut = normalized.lastIndexOf("/");
  return cut === -1 ? "" : normalized.slice(0, cut);
}

function fileNameOf(path: string): string {
  const normalized = normalizePath(path);
  return normalized.slice(normalized.lastIndexOf("/") + 1);
}

/** Claude Desktop's own executable (Windows `...\AnthropicClaude\...\claude.exe`, macOS `Claude.app`). */
export function isClaudeDesktopExe(path: string | null): boolean {
  if (!path) {
    return false;
  }
  const normalized = normalizePath(path);
  return (
    (normalized.includes("/anthropicclaude/") && fileNameOf(normalized) === "claude.exe") ||
    normalized.includes("/claude.app/contents/")
  );
}

/** The Claude Code copy that Claude Desktop's Code tab runs (`%APPDATA%\Claude\claude-code\<ver>\claude.exe`). */
export function isDesktopClaudeCodeExe(path: string | null): boolean {
  return Boolean(path) && normalizePath(path ?? "").includes("/claude/claude-code/");
}

/** Friendly name for a raw `clientInfo.name`; the raw name itself when unknown. */
export function friendlyClientName(rawName: string): string {
  const key = rawName.trim().toLowerCase();
  const known = KNOWN_CLIENTS[key];
  if (known) {
    return known;
  }
  const prefixed = CLIENT_PREFIXES.find(([prefix]) => key.startsWith(prefix));
  return prefixed ? prefixed[1] : rawName.trim();
}

/**
 * Who runs a sidecar. `named` carries a display label; the other kinds are
 * translated by the UI: `app` is the child this app started, `pending` a
 * current sidecar whose client has not sent `initialize` yet, and
 * `unknownOlder` a sidecar that predates client tracking.
 */
export type SidecarClient =
  | { kind: "named"; label: string }
  | { kind: "app" }
  | { kind: "pending" }
  | { kind: "unknownOlder" };

export function sidecarClient(
  sidecar: SidecarHeartbeat,
  managedSidecarPid: number | null = null
): SidecarClient {
  if (managedSidecarPid !== null && sidecar.pid === managedSidecarPid) {
    return { kind: "app" };
  }
  const rawName = sidecar.clientName?.trim();
  if (rawName) {
    const label = friendlyClientName(rawName);
    // Claude Code in a terminal and in Claude Desktop's Code tab both send
    // "claude-code"; the parent executable tells them apart.
    if (label === "Claude Code" && isDesktopClaudeCodeExe(sidecar.parentExePath)) {
      return { kind: "named", label: CLAUDE_CODE_IN_DESKTOP_LABEL };
    }
    return { kind: "named", label };
  }
  // No clientInfo yet: fall back to the parent process when it is telling.
  if (isClaudeDesktopExe(sidecar.parentExePath)) {
    return { kind: "named", label: "Claude Desktop" };
  }
  if (isDesktopClaudeCodeExe(sidecar.parentExePath)) {
    return { kind: "named", label: CLAUDE_CODE_IN_DESKTOP_LABEL };
  }
  if (!sidecar.exePath && !sidecar.parentExePath) {
    return { kind: "unknownOlder" };
  }
  return { kind: "pending" };
}

/**
 * True when the sidecar runs from a directory other than the one this install
 * launches (e.g. a workspace `target\debug` build). Unknown paths are never
 * flagged. Compared by directory so the installer's renamed
 * `sheet-port-mcp.old-N.exe` copies still count as this install.
 */
export function isDevBuild(sidecar: SidecarHeartbeat, bundledSidecarPath: string | null): boolean {
  if (!sidecar.exePath || !bundledSidecarPath) {
    return false;
  }
  return directoryOf(sidecar.exePath) !== directoryOf(bundledSidecarPath);
}
