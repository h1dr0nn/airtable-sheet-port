import type { DatabaseSync } from "node:sqlite";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { openSheetPortDb } from "../src/index.js";
import { makeTempDb, type TempDb } from "./tempDb.js";

describe("openSheetPortDb", () => {
  let temp: TempDb;
  let db: DatabaseSync;

  beforeEach(() => {
    temp = makeTempDb();
    db = openSheetPortDb(temp.path);
  });

  afterEach(() => {
    db.close();
    temp.cleanup();
  });

  it("applies the schema and seed on first open", () => {
    // Arrange: beforeEach opened a fresh DB file.

    // Act
    const sources = db.prepare("SELECT id FROM sources ORDER BY id").all() as Array<{ id: string }>;
    const rules = db
      .prepare("SELECT source_id, table_id FROM permission_rules")
      .all() as Array<{ source_id: string; table_id: string | null }>;
    const records = db.prepare("SELECT COUNT(*) AS n FROM mock_records").get() as { n: number };
    const seeded = db.prepare("SELECT value FROM meta WHERE key = 'seeded'").get() as { value: string };

    // Assert
    expect(sources.map((row) => row.id)).toEqual(["google-placeholder", "mock-source", "provider-placeholder"]);
    expect(rules).toEqual([{ source_id: "mock-source", table_id: "customers" }]);
    expect(records.n).toBe(3);
    expect(seeded.value).toBe("1");
  });

  it("is idempotent across two opens and does not duplicate seed rows", () => {
    // Arrange
    db.close();

    // Act: second open of the same file must re-apply schema harmlessly.
    db = openSheetPortDb(temp.path);

    // Assert
    const sources = db.prepare("SELECT COUNT(*) AS n FROM sources").get() as { n: number };
    const rules = db.prepare("SELECT COUNT(*) AS n FROM permission_rules").get() as { n: number };
    const records = db.prepare("SELECT COUNT(*) AS n FROM mock_records").get() as { n: number };
    expect(sources.n).toBe(3);
    expect(rules.n).toBe(1);
    expect(records.n).toBe(3);
  });

  it("does not re-seed deleted rows once the seeded marker is set", () => {
    // Arrange: user-visible deletions must survive process restarts.
    db.prepare("DELETE FROM mock_records WHERE record_id = 'rec_seed_1'").run();
    db.close();

    // Act
    db = openSheetPortDb(temp.path);

    // Assert
    const records = db.prepare("SELECT COUNT(*) AS n FROM mock_records").get() as { n: number };
    expect(records.n).toBe(2);
  });
});
