import { DatabaseSync } from "node:sqlite";
import { resolveDbPath } from "./paths.js";
import { SCHEMA_SQL, SEED_SQL } from "./sql.js";

/**
 * Opens the shared database, applies the connection pragmas required by
 * docs/ipc.md, and runs schema + first-run seed. Whichever process (desktop
 * app or MCP sidecar) opens the DB first performs the setup; both steps are
 * idempotent.
 */
export function openSheetPortDb(dbPath: string = resolveDbPath()): DatabaseSync {
  const db = new DatabaseSync(dbPath);
  db.exec("PRAGMA journal_mode=WAL;");
  db.exec("PRAGMA busy_timeout=5000;");
  db.exec("PRAGMA foreign_keys=ON;");
  db.exec(SCHEMA_SQL);
  if (!isSeeded(db)) {
    db.exec(SEED_SQL);
  }
  return db;
}

function isSeeded(db: DatabaseSync): boolean {
  const row = db.prepare("SELECT value FROM meta WHERE key = 'seeded'").get();
  return row !== undefined;
}
