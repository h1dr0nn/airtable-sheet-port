import type { DatabaseSync } from "node:sqlite";
import type { PermissionRule } from "@sheet-port/shared";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { openSheetPortDb, PermissionStore } from "../src/index.js";
import { makeTempDb, type TempDb } from "./tempDb.js";

const SOURCE = "src-a";

function makeRule(overrides: Partial<PermissionRule> = {}): PermissionRule {
  return {
    sourceId: SOURCE,
    read: true,
    write: true,
    deleteRecords: false,
    requireConfirmationFor: ["update"],
    ...overrides
  };
}

describe("PermissionStore", () => {
  let temp: TempDb;
  let db: DatabaseSync;
  let store: PermissionStore;

  beforeEach(() => {
    temp = makeTempDb();
    db = openSheetPortDb(temp.path);
    store = new PermissionStore(db);
  });

  afterEach(() => {
    db.close();
    temp.cleanup();
  });

  it("resolves a table-specific rule over the source-wide rule", () => {
    // Arrange
    store.upsert(makeRule({ write: true }));
    store.upsert(makeRule({ tableId: "t1", write: false }));

    // Act
    const tableRule = store.get(SOURCE, "t1");
    const fallbackRule = store.get(SOURCE, "other-table");

    // Assert
    expect(tableRule?.tableId).toBe("t1");
    expect(tableRule?.write).toBe(false);
    expect(fallbackRule?.tableId).toBeUndefined();
    expect(fallbackRule?.write).toBe(true);
  });

  it("returns the source-wide rule when no table id is given", () => {
    // Arrange
    store.upsert(makeRule({ read: false }));
    store.upsert(makeRule({ tableId: "t1", read: true }));

    // Act
    const rule = store.get(SOURCE);

    // Assert
    expect(rule?.tableId).toBeUndefined();
    expect(rule?.read).toBe(false);
  });

  it("upserts the NULL table_id rule in place instead of duplicating it", () => {
    // Arrange
    const first = store.upsert(makeRule({ write: true, requireConfirmationFor: [] }));

    // Act
    const second = store.upsert(makeRule({ write: false, requireConfirmationFor: ["append", "bulk_update"] }));

    // Assert: same row id, updated values, exactly one rule for the source.
    expect(second.id).toBe(first.id);
    expect(second.write).toBe(false);
    expect(second.requireConfirmationFor).toEqual(["append", "bulk_update"]);
    const forSource = store.list().filter((rule) => rule.sourceId === SOURCE);
    expect(forSource).toHaveLength(1);
  });

  it("deletes a rule by id and throws for an unknown id", () => {
    // Arrange
    const stored = store.upsert(makeRule());

    // Act
    store.delete(stored.id);

    // Assert
    expect(store.get(SOURCE)).toBeUndefined();
    expect(() => store.delete(stored.id)).toThrow(`Unknown permission rule ${stored.id}`);
  });
});
