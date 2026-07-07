import { beforeEach, describe, expect, it } from "vitest";
import { createWorkbenchDemo, type WorkbenchDemoApi } from "../workbenchDemo.js";

// The demo Workbench backend is pure in-memory logic, so tests drive it with an
// immediate (no-op) delay and assert the mutations persist across reads.

const immediateDelay = () => Promise.resolve();

describe("workbench demo backend", () => {
  let api: WorkbenchDemoApi;

  beforeEach(() => {
    api = createWorkbenchDemo({ delay: immediateDelay });
  });

  it("seeds two folders, three foldered sheets, and one ungrouped sheet", async () => {
    const tree = await api.workbenchTree();
    expect(tree.folders.map((folder) => folder.name)).toEqual(["Game Config", "Live Ops"]);
    expect(tree.items).toHaveLength(4);
    expect(tree.items.filter((item) => item.folderId === null)).toHaveLength(1);
  });

  it("lists sheet tabs and reads a seeded grid", async () => {
    const tabs = await api.listSheetTabs("wbi_core");
    expect(tabs.map((tab) => tab.title)).toEqual(["Constants", "Formulas", "Progression"]);

    const grid = await api.readSheet("wbi_core", "0", null, null);
    expect(grid.columns[0]?.title).toBe("Key");
    expect(grid.totalRows).toBe(grid.rows.length);
    expect(grid.rows[0]?.[grid.columns[0]!.id]).toBe("MAX_LEVEL");
  });

  it("persists a cell edit", async () => {
    const before = await api.readSheet("wbi_core", "0", null, null);
    const columnId = before.columns[1]!.id; // "Value" column

    await api.updateCell("wbi_core", "0", 0, columnId, "99");

    const after = await api.readSheet("wbi_core", "0", null, null);
    expect(after.rows[0]?.[columnId]).toBe("99");
  });

  it("rejects a cell edit outside the row range", async () => {
    await expect(api.updateCell("wbi_core", "0", 9999, "c0", "x")).rejects.toThrow();
  });

  it("appends a row and returns its index", async () => {
    const before = await api.readSheet("wbi_core", "0", null, null);
    const result = await api.appendSheetRow("wbi_core", "0", { c0: "NEW_KEY" });

    expect(result.rowIndex).toBe(before.totalRows);

    const after = await api.readSheet("wbi_core", "0", null, null);
    expect(after.totalRows).toBe(before.totalRows + 1);
    expect(after.rows[result.rowIndex]?.c0).toBe("NEW_KEY");
  });

  it("respects read limit and offset", async () => {
    const page = await api.readSheet("wbi_core", "0", 3, 2);
    expect(page.rows).toHaveLength(3);
    // totalRows always reflects the full sheet, not the page.
    const full = await api.readSheet("wbi_core", "0", null, null);
    expect(page.totalRows).toBe(full.totalRows);
    expect(page.rows[0]).toEqual(full.rows[2]);
  });

  it("creates a folder at the end of the tree", async () => {
    const folder = await api.createFolder("Tuning");
    const tree = await api.workbenchTree();
    expect(tree.folders.map((f) => f.name)).toContain("Tuning");
    expect(folder.position).toBeGreaterThan(0);
  });

  it("rejects an empty folder name", async () => {
    await expect(api.createFolder("   ")).rejects.toThrow();
  });

  it("renames a folder", async () => {
    await api.renameFolder("fld_live_ops", "Operations");
    const tree = await api.workbenchTree();
    expect(tree.folders.find((f) => f.id === "fld_live_ops")?.name).toBe("Operations");
  });

  it("deletes a folder and reparents its sheets to ungrouped", async () => {
    await api.deleteFolder("fld_game_config");
    const tree = await api.workbenchTree();
    expect(tree.folders.some((f) => f.id === "fld_game_config")).toBe(false);
    const reparented = tree.items.filter((item) => item.folderId === null);
    // The two Game Config sheets join the pre-existing ungrouped sheet.
    expect(reparented).toHaveLength(3);
  });

  it("adds a spreadsheet from a URL and creates two starter tabs", async () => {
    const item = await api.addSpreadsheet({
      folderId: "fld_live_ops",
      urlOrId: "https://docs.google.com/spreadsheets/d/1AbCdEfGhIj/edit#gid=0"
    });
    expect(item.spreadsheetId).toBe("1AbCdEfGhIj");
    expect(item.folderId).toBe("fld_live_ops");

    const tabs = await api.listSheetTabs(item.id);
    expect(tabs).toHaveLength(2);

    const tree = await api.workbenchTree();
    expect(tree.items.some((i) => i.id === item.id)).toBe(true);
  });

  it("adds a spreadsheet from a bare id", async () => {
    const item = await api.addSpreadsheet({ folderId: null, urlOrId: "BareSheetId123" });
    expect(item.spreadsheetId).toBe("BareSheetId123");
    expect(item.folderId).toBeNull();
  });

  it("rejects adding a spreadsheet with an empty input", async () => {
    await expect(api.addSpreadsheet({ folderId: null, urlOrId: "  " })).rejects.toThrow();
  });

  it("moves a spreadsheet to another folder", async () => {
    await api.moveWorkbenchItem("wbi_economy", "fld_live_ops");
    const tree = await api.workbenchTree();
    expect(tree.items.find((i) => i.id === "wbi_economy")?.folderId).toBe("fld_live_ops");
  });

  it("removes a spreadsheet from the tree", async () => {
    await api.removeWorkbenchItem("wbi_units");
    const tree = await api.workbenchTree();
    expect(tree.items.some((i) => i.id === "wbi_units")).toBe(false);
    await expect(api.listSheetTabs("wbi_units")).rejects.toThrow();
  });
});
