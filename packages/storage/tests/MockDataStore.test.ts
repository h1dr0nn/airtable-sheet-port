import type { DatabaseSync } from "node:sqlite";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { MockDataStore, openSheetPortDb } from "../src/index.js";
import { makeTempDb, type TempDb } from "./tempDb.js";

const SOURCE = "mock-source";
const TABLE = "customers";

describe("MockDataStore", () => {
  let temp: TempDb;
  let db: DatabaseSync;
  let store: MockDataStore;

  beforeEach(() => {
    temp = makeTempDb();
    db = openSheetPortDb(temp.path);
    store = new MockDataStore(db);
  });

  afterEach(() => {
    db.close();
    temp.cleanup();
  });

  it("paginates records by position and reports the unpaged total", () => {
    // Act
    const page = store.listRecords(SOURCE, TABLE, { limit: 2, offset: 1 });

    // Assert: seed has 3 records ordered rec_seed_1..3.
    expect(page.records.map((record) => record.id)).toEqual(["rec_seed_2", "rec_seed_3"]);
    expect(page.total).toBe(3);
  });

  it("returns all records when no pagination options are given", () => {
    // Act
    const page = store.listRecords(SOURCE, TABLE);

    // Assert
    expect(page.records).toHaveLength(3);
    expect(page.total).toBe(3);
  });

  it("appends records with generated rec_ ids after the existing ones", () => {
    // Act
    const appended = store.appendRecords(SOURCE, TABLE, [{ Name: "Delta" }, { Name: "Echo" }]);

    // Assert
    expect(appended).toHaveLength(2);
    for (const record of appended) {
      expect(record.id).toMatch(/^rec_/);
    }
    const page = store.listRecords(SOURCE, TABLE);
    expect(page.total).toBe(5);
    // Position ordering: the new records come last, in append order.
    expect(page.records.slice(3).map((record) => record.id)).toEqual(appended.map((record) => record.id));
    expect(page.records[3]?.fields).toEqual({ Name: "Delta" });
    expect(page.records[4]?.fields).toEqual({ Name: "Echo" });
  });

  it("shallow-merges patch fields into the stored record", () => {
    // Act
    const updated = store.updateRecords(SOURCE, TABLE, [{ recordId: "rec_seed_1", fields: { Seats: 99 } }]);

    // Assert: untouched fields survive, patched field wins.
    expect(updated).toHaveLength(1);
    expect(updated[0]?.fields).toMatchObject({ Name: "Aurora Labs", Seats: 99, Plan: "pro" });
    const page = store.listRecords(SOURCE, TABLE, { limit: 1 });
    expect(page.records[0]?.fields.Seats).toBe(99);
  });

  it("skips unknown record ids on update", () => {
    // Act
    const updated = store.updateRecords(SOURCE, TABLE, [{ recordId: "rec_missing", fields: { Seats: 1 } }]);

    // Assert
    expect(updated).toEqual([]);
  });

  it("reports unknown tables as undefined schema and an empty page", () => {
    // Act
    const schema = store.getTable(SOURCE, "no-such-table");
    const page = store.listRecords(SOURCE, "no-such-table");

    // Assert: the connector layer turns this into an "Unknown mock table" error.
    expect(schema).toBeUndefined();
    expect(page).toEqual({ records: [], total: 0 });
  });

  it("exposes the seeded table schema", () => {
    // Act
    const tables = store.listTables(SOURCE);
    const schema = store.getTable(SOURCE, TABLE);

    // Assert
    expect(tables).toEqual([{ sourceId: SOURCE, tableId: TABLE, name: "Customers" }]);
    expect(schema?.fields.map((field) => field.name)).toEqual(["Name", "Email", "Plan", "Seats", "Active"]);
  });
});
