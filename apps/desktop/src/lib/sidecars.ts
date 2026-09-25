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
