import type { DatabaseSync } from "node:sqlite";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { HeartbeatStore, openSheetPortDb } from "../src/index.js";
import { makeTempDb, type TempDb } from "./tempDb.js";

const TTL_MS = 30000;
const STALE_AGE_MS = 60000;

describe("HeartbeatStore", () => {
  let temp: TempDb;
  let db: DatabaseSync;
  let store: HeartbeatStore;

  beforeEach(() => {
    temp = makeTempDb();
    db = openSheetPortDb(temp.path);
    store = new HeartbeatStore(db);
  });

  afterEach(() => {
    db.close();
    temp.cleanup();
  });

  function insertStaleRow(pid: number): void {
    const staleIso = new Date(Date.now() - STALE_AGE_MS).toISOString();
    db.prepare("INSERT INTO mcp_heartbeat (pid, started_at, last_seen) VALUES (?, ?, ?)").run(pid, staleIso, staleIso);
  }

  function countRows(): number {
    const row = db.prepare("SELECT COUNT(*) AS n FROM mcp_heartbeat").get() as { n: number };
    return row.n;
  }

  it("upserts its own row in place on repeated heartbeats", () => {
    // Act
    store.upsertOwn(111);
    store.upsertOwn(111);

    // Assert
    expect(countRows()).toBe(1);
    const status = store.isAlive(TTL_MS);
    expect(status.running).toBe(true);
    expect(status.pid).toBe(111);
    expect(status.lastSeen).not.toBeNull();
  });

  it("deletes stale rows but keeps fresh ones", () => {
    // Arrange
    insertStaleRow(222);
    store.upsertOwn(111);

    // Act
    store.deleteStale(TTL_MS);

    // Assert
    expect(countRows()).toBe(1);
    expect(store.isAlive(TTL_MS).pid).toBe(111);
  });

  it("reports not running when only stale heartbeats exist", () => {
    // Arrange
    insertStaleRow(222);

    // Act
    const status = store.isAlive(TTL_MS);

    // Assert
    expect(status).toEqual({ running: false, pid: null, lastSeen: null });
  });

  it("removes its own row on shutdown", () => {
    // Arrange
    store.upsertOwn(111);

    // Act
    store.deleteOwn(111);

    // Assert
    expect(store.isAlive(TTL_MS).running).toBe(false);
    expect(countRows()).toBe(0);
  });
});
