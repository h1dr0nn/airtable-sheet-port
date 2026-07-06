import type { DatabaseSync } from "node:sqlite";
import { isoBefore, nowIso } from "../util.js";

export type HeartbeatStatus = {
  running: boolean;
  pid: number | null;
  lastSeen: string | null;
};

export class HeartbeatStore {
  constructor(private readonly db: DatabaseSync) {}

  upsertOwn(pid: number): void {
    const now = nowIso();
    this.db
      .prepare(
        `INSERT INTO mcp_heartbeat (pid, started_at, last_seen) VALUES (?, ?, ?)
         ON CONFLICT(pid) DO UPDATE SET last_seen = excluded.last_seen`
      )
      .run(pid, now, now);
  }

  deleteStale(ttlMs: number): void {
    this.db.prepare("DELETE FROM mcp_heartbeat WHERE last_seen < ?").run(isoBefore(ttlMs));
  }

  deleteOwn(pid: number): void {
    this.db.prepare("DELETE FROM mcp_heartbeat WHERE pid = ?").run(pid);
  }

  isAlive(ttlMs: number): HeartbeatStatus {
    const row = this.db
      .prepare("SELECT pid, last_seen FROM mcp_heartbeat WHERE last_seen >= ? ORDER BY last_seen DESC LIMIT 1")
      .get(isoBefore(ttlMs)) as { pid: number; last_seen: string } | undefined;
    if (!row) {
      return { running: false, pid: null, lastSeen: null };
    }
    return { running: true, pid: row.pid, lastSeen: row.last_seen };
  }
}
