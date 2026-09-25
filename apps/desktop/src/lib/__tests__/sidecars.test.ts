import { describe, expect, it } from "vitest";
import type { SidecarHeartbeat } from "../ipc";
import {
  CLAUDE_CODE_IN_DESKTOP_LABEL,
  friendlyClientName,
  isDevBuild,
  isSidecarOutdated,
  normalizeVersion,
  outdatedSidecars,
  outdatedVersionLabel,
  sidecarClient
} from "../sidecars";

const LAST_SEEN = "2026-09-25T00:00:00.000Z";

function sidecar(
  pid: number,
  version: string | null,
  extra: Partial<SidecarHeartbeat> = {}
): SidecarHeartbeat {
  return {
    pid,
    version,
    lastSeen: LAST_SEEN,
    clientName: null,
    clientVersion: null,
    exePath: null,
    parentExePath: null,
    ...extra
  };
}

describe("normalizeVersion", () => {
  it("strips a leading v and trailing labels", () => {
    expect(normalizeVersion("v2.2.1")).toBe("2.2.1");
    expect(normalizeVersion("0.0.1 (browser demo)")).toBe("0.0.1");
    expect(normalizeVersion(" 2.2.1 ")).toBe("2.2.1");
  });
});

describe("isSidecarOutdated", () => {
  it("is current when the versions match", () => {
    expect(isSidecarOutdated(sidecar(1, "2.2.1"), "2.2.1")).toBe(false);
    expect(isSidecarOutdated(sidecar(1, "2.2.1"), "v2.2.1")).toBe(false);
  });

  it("is outdated when the version differs", () => {
    expect(isSidecarOutdated(sidecar(1, "2.1.0"), "2.2.1")).toBe(true);
  });

  it("is outdated when the sidecar reports no version", () => {
    expect(isSidecarOutdated(sidecar(1, null), "2.2.1")).toBe(true);
    expect(isSidecarOutdated(sidecar(1, ""), "2.2.1")).toBe(true);
  });
});

describe("outdatedSidecars / outdatedVersionLabel", () => {
  it("returns only the outdated sidecars", () => {
    const list = [sidecar(1, "2.2.1"), sidecar(2, "2.1.0"), sidecar(3, null)];
    expect(outdatedSidecars(list, "2.2.1").map((item) => item.pid)).toEqual([2, 3]);
    expect(outdatedSidecars([sidecar(1, "2.2.1")], "2.2.1")).toEqual([]);
  });

  it("names a single old version", () => {
    expect(outdatedVersionLabel([sidecar(2, "v2.1.0"), sidecar(4, "2.1.0")])).toBe("2.1.0");
  });

  it("falls back to null for unknown or mixed versions", () => {
    expect(outdatedVersionLabel([sidecar(3, null)])).toBeNull();
    expect(outdatedVersionLabel([sidecar(2, "2.1.0"), sidecar(3, null)])).toBeNull();
    expect(outdatedVersionLabel([sidecar(2, "2.1.0"), sidecar(5, "2.0.0")])).toBeNull();
    expect(outdatedVersionLabel([])).toBeNull();
  });
});

const INSTALLED = "D:\\Utilities\\Airtable - Sheet Port\\sheet-port-mcp.exe";
const DEV_BUILD = "D:\\Projects\\airtable-sheet-port\\target\\debug\\sheet-port-mcp.exe";
const DESKTOP_EXE = "C:\\Users\\me\\AppData\\Local\\AnthropicClaude\\app-2.9939.2\\claude.exe";
const CODE_TAB_EXE = "C:\\Users\\me\\AppData\\Roaming\\Claude\\claude-code\\2.1.281\\claude.exe";
const CLI_EXE = "C:\\Users\\me\\.local\\bin\\claude.exe";

describe("friendlyClientName", () => {
  it("maps known clientInfo names", () => {
    expect(friendlyClientName("claude-code")).toBe("Claude Code");
    expect(friendlyClientName("claude-ai")).toBe("Claude Desktop");
    expect(friendlyClientName("Claude Desktop")).toBe("Claude Desktop");
    expect(friendlyClientName("cursor-vscode")).toBe("Cursor");
    expect(friendlyClientName("Visual Studio Code")).toBe("VS Code");
    expect(friendlyClientName("codex-mcp-client")).toBe("Codex");
    expect(friendlyClientName("windsurf-next")).toBe("Windsurf");
  });

  it("falls back to the raw name", () => {
    expect(friendlyClientName("smoke")).toBe("smoke");
    expect(friendlyClientName("  my-agent ")).toBe("my-agent");
  });
});

describe("sidecarClient", () => {
  it("labels named clients", () => {
    expect(sidecarClient(sidecar(1, "2.3.0", { clientName: "claude-code", parentExePath: CLI_EXE })))
      .toEqual({ kind: "named", label: "Claude Code" });
    expect(sidecarClient(sidecar(1, "2.3.0", { clientName: "claude-ai", parentExePath: DESKTOP_EXE })))
      .toEqual({ kind: "named", label: "Claude Desktop" });
    expect(sidecarClient(sidecar(1, "2.3.0", { clientName: "smoke" })))
      .toEqual({ kind: "named", label: "smoke" });
  });

  it("tells Claude Desktop's Code tab from Claude Code in a terminal", () => {
    expect(sidecarClient(sidecar(1, "2.3.0", { clientName: "claude-code", parentExePath: CODE_TAB_EXE })))
      .toEqual({ kind: "named", label: CLAUDE_CODE_IN_DESKTOP_LABEL });
  });

  it("infers the client from the parent before initialize", () => {
    expect(sidecarClient(sidecar(1, "2.3.0", { exePath: INSTALLED, parentExePath: DESKTOP_EXE })))
      .toEqual({ kind: "named", label: "Claude Desktop" });
    expect(sidecarClient(sidecar(1, "2.3.0", { exePath: INSTALLED, parentExePath: CLI_EXE })))
      .toEqual({ kind: "pending" });
  });

  it("marks the app's own managed child", () => {
    expect(sidecarClient(sidecar(7, "2.3.0", { exePath: INSTALLED }), 7)).toEqual({ kind: "app" });
    expect(sidecarClient(sidecar(8, "2.3.0", { exePath: INSTALLED }), 7)).toEqual({ kind: "pending" });
  });

  it("reports sidecars without client tracking as older", () => {
    expect(sidecarClient(sidecar(1, "2.2.1"))).toEqual({ kind: "unknownOlder" });
    expect(sidecarClient(sidecar(1, null), null)).toEqual({ kind: "unknownOlder" });
  });
});

describe("isDevBuild", () => {
  it("flags sidecars outside this install's directory", () => {
    expect(isDevBuild(sidecar(1, "2.3.0", { exePath: DEV_BUILD }), INSTALLED)).toBe(true);
  });

  it("accepts this install, renamed update copies, and other spellings", () => {
    expect(isDevBuild(sidecar(1, "2.3.0", { exePath: INSTALLED }), INSTALLED)).toBe(false);
    expect(
      isDevBuild(
        sidecar(1, "2.2.1", { exePath: "d:/utilities/airtable - sheet port/sheet-port-mcp.old-1.exe" }),
        INSTALLED
      )
    ).toBe(false);
  });

  it("never flags unknown paths", () => {
    expect(isDevBuild(sidecar(1, "2.2.1"), INSTALLED)).toBe(false);
    expect(isDevBuild(sidecar(1, "2.3.0", { exePath: DEV_BUILD }), null)).toBe(false);
  });
});
