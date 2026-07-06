import { mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import process from "node:process";

/** Absolute DB file path override used by tests and smoke scripts. */
export const DB_ENV_VAR = "SHEET_PORT_DB";

const DB_DIR_NAME = "sheet-port";
const DB_FILE_NAME = "sheet-port.db";

/**
 * Resolves the shared SQLite path per docs/ipc.md and ensures the parent
 * directory exists so opening the database never fails on a fresh machine.
 */
export function resolveDbPath(): string {
  const override = process.env[DB_ENV_VAR];
  const dbPath = override && override.length > 0 ? override : join(dataDir(), DB_DIR_NAME, DB_FILE_NAME);
  mkdirSync(dirname(dbPath), { recursive: true });
  return dbPath;
}

function dataDir(): string {
  if (process.platform === "win32") {
    return process.env.APPDATA ?? join(homedir(), "AppData", "Roaming");
  }
  if (process.platform === "darwin") {
    return join(homedir(), "Library", "Application Support");
  }
  const xdgDataHome = process.env.XDG_DATA_HOME;
  return xdgDataHome && xdgDataHome.length > 0 ? xdgDataHome : join(homedir(), ".local", "share");
}
