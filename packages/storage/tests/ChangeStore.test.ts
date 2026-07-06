import type { DatabaseSync } from "node:sqlite";
import type { ChangePayload } from "@sheet-port/core";
import type { PendingChange } from "@sheet-port/shared";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { ChangeStore, openSheetPortDb } from "../src/index.js";
import { makeTempDb, type TempDb } from "./tempDb.js";

function makeChange(overrides: Partial<PendingChange> = {}): PendingChange {
  return {
    id: "chg_test_1",
    sourceId: "mock-source",
    tableId: "customers",
    type: "update",
    createdAt: new Date().toISOString(),
    status: "pending",
    requiresConfirmation: true,
    diff: [{ recordId: "rec_seed_1", before: { Seats: 24 }, after: { Seats: 25 } }],
    ...overrides
  };
}

const PAYLOAD: ChangePayload = { type: "update", patches: [{ recordId: "rec_seed_1", fields: { Seats: 25 } }] };

describe("ChangeStore", () => {
  let temp: TempDb;
  let db: DatabaseSync;
  let store: ChangeStore;

  beforeEach(() => {
    temp = makeTempDb();
    db = openSheetPortDb(temp.path);
    store = new ChangeStore(db);
  });

  afterEach(() => {
    db.close();
    temp.cleanup();
  });

  it("round-trips a change through insert and get", () => {
    // Arrange
    const change = makeChange();

    // Act
    store.insert(change, PAYLOAD);
    const loaded = store.get(change.id);

    // Assert
    expect(loaded).toEqual(change);
    expect(store.getPayload(change.id)).toEqual(PAYLOAD);
  });

  it("never exposes the payload on public change objects", () => {
    // Arrange
    const change = makeChange();
    store.insert(change, PAYLOAD);

    // Act
    const loaded = store.get(change.id);
    const listed = store.list();

    // Assert
    expect(loaded).not.toHaveProperty("payload");
    expect(listed).toHaveLength(1);
    expect(listed[0]).not.toHaveProperty("payload");
  });

  it("guards transitions by the expected from-status", () => {
    // Arrange
    const change = makeChange();
    store.insert(change, PAYLOAD);

    // Act
    const first = store.transition(change.id, "pending", "approved", "user");
    const second = store.transition(change.id, "pending", "approved", "user");

    // Assert: approving twice must fail the second time.
    expect(first).toBe(true);
    expect(second).toBe(false);
    const loaded = store.get(change.id);
    expect(loaded?.status).toBe("approved");
    expect(loaded?.decidedBy).toBe("user");
    expect(loaded?.decidedAt).toBeDefined();
  });

  it("marks committed only from the approved status", () => {
    // Arrange
    const change = makeChange();
    store.insert(change, PAYLOAD);

    // Act + Assert: pending -> committed is not allowed.
    expect(store.markCommitted(change.id)).toBe(false);

    store.transition(change.id, "pending", "approved", "user");
    expect(store.markCommitted(change.id)).toBe(true);
    const loaded = store.get(change.id);
    expect(loaded?.status).toBe("committed");
    expect(loaded?.committedAt).toBeDefined();

    // Double commit must fail.
    expect(store.markCommitted(change.id)).toBe(false);
  });

  it("filters list by status, newest first", () => {
    // Arrange
    store.insert(makeChange({ id: "chg_a", createdAt: "2026-01-01T00:00:00.000Z" }), PAYLOAD);
    store.insert(makeChange({ id: "chg_b", createdAt: "2026-01-02T00:00:00.000Z" }), PAYLOAD);
    store.transition("chg_a", "pending", "approved", "user");

    // Act
    const all = store.list();
    const pending = store.list("pending");

    // Assert
    expect(all.map((change) => change.id)).toEqual(["chg_b", "chg_a"]);
    expect(pending.map((change) => change.id)).toEqual(["chg_b"]);
  });
});
