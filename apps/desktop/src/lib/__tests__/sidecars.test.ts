import { describe, expect, it } from "vitest";
import type { SidecarHeartbeat } from "../ipc";
import {
  isSidecarOutdated,
  normalizeVersion,
  outdatedSidecars,
  outdatedVersionLabel
} from "../sidecars";

const LAST_SEEN = "2026-09-25T00:00:00.000Z";

function sidecar(pid: number, version: string | null): SidecarHeartbeat {
  return { pid, version, lastSeen: LAST_SEEN };
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
