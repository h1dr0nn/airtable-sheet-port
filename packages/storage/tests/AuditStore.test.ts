import type { DatabaseSync } from "node:sqlite";
import type { AuditEvent } from "@sheet-port/shared";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { AuditStore, openSheetPortDb } from "../src/index.js";
import { makeTempDb, type TempDb } from "./tempDb.js";

function makeEvent(id: string, timestamp: string, overrides: Partial<AuditEvent> = {}): AuditEvent {
  return { id, timestamp, actor: "agent", action: "read_table", ...overrides };
}

describe("AuditStore", () => {
  let temp: TempDb;
  let db: DatabaseSync;
  let store: AuditStore;

  beforeEach(() => {
    temp = makeTempDb();
    db = openSheetPortDb(temp.path);
    store = new AuditStore(db);
  });

  afterEach(() => {
    db.close();
    temp.cleanup();
  });

  it("lists events newest first with limit and offset", () => {
    // Arrange
    store.insert(makeEvent("evt_1", "2026-01-01T00:00:00.000Z"));
    store.insert(makeEvent("evt_2", "2026-01-02T00:00:00.000Z"));
    store.insert(makeEvent("evt_3", "2026-01-03T00:00:00.000Z"));

    // Act + Assert
    expect(store.list(10).map((event) => event.id)).toEqual(["evt_3", "evt_2", "evt_1"]);
    expect(store.list(1).map((event) => event.id)).toEqual(["evt_3"]);
    expect(store.list(2, 1).map((event) => event.id)).toEqual(["evt_2", "evt_1"]);
  });

  it("breaks same-timestamp ties by insertion order, newest insert first", () => {
    // Arrange
    const timestamp = "2026-01-01T00:00:00.000Z";
    store.insert(makeEvent("evt_first", timestamp));
    store.insert(makeEvent("evt_second", timestamp));

    // Act
    const events = store.list(10);

    // Assert
    expect(events.map((event) => event.id)).toEqual(["evt_second", "evt_first"]);
  });

  it("round-trips optional source, table, and metadata fields", () => {
    // Arrange
    const full = makeEvent("evt_full", "2026-01-01T00:00:00.000Z", {
      sourceId: "mock-source",
      tableId: "customers",
      metadata: { count: 2 }
    });
    const bare = makeEvent("evt_bare", "2026-01-02T00:00:00.000Z");
    store.insert(full);
    store.insert(bare);

    // Act
    const events = store.list(10);

    // Assert: absent optional columns stay absent on the mapped object.
    expect(events[1]).toEqual(full);
    expect(events[0]).toEqual(bare);
    expect(events[0]).not.toHaveProperty("metadata");
    expect(events[0]).not.toHaveProperty("sourceId");
  });
});
