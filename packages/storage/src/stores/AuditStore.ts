import type { DatabaseSync } from "node:sqlite";
import type { AuditStorePort } from "@sheet-port/core";
import type { AuditEvent } from "@sheet-port/shared";
import { parseJsonColumn } from "../util.js";

type AuditEventRow = {
  id: string;
  timestamp: string;
  actor: string;
  action: string;
  source_id: string | null;
  table_id: string | null;
  metadata: string | null;
};

const AUDIT_COLUMNS = "id, timestamp, actor, action, source_id, table_id, metadata";

export class AuditStore implements AuditStorePort {
  constructor(private readonly db: DatabaseSync) {}

  insert(event: AuditEvent): void {
    this.db
      .prepare(
        `INSERT INTO audit_events (id, timestamp, actor, action, source_id, table_id, metadata)
         VALUES (?, ?, ?, ?, ?, ?, ?)`
      )
      .run(
        event.id,
        event.timestamp,
        event.actor,
        event.action,
        event.sourceId ?? null,
        event.tableId ?? null,
        event.metadata !== undefined ? JSON.stringify(event.metadata) : null
      );
  }

  list(limit: number, offset = 0): AuditEvent[] {
    // rowid tiebreaker keeps same-millisecond events in insertion order.
    const rows = this.db
      .prepare(`SELECT ${AUDIT_COLUMNS} FROM audit_events ORDER BY timestamp DESC, rowid DESC LIMIT ? OFFSET ?`)
      .all(limit, offset) as AuditEventRow[];
    return rows.map(mapAuditRow);
  }
}

function mapAuditRow(row: AuditEventRow): AuditEvent {
  return {
    id: row.id,
    timestamp: row.timestamp,
    actor: row.actor as AuditEvent["actor"],
    action: row.action,
    ...(row.source_id !== null ? { sourceId: row.source_id } : {}),
    ...(row.table_id !== null ? { tableId: row.table_id } : {}),
    ...(row.metadata !== null ? { metadata: parseJsonColumn<Record<string, unknown>>(row.metadata) } : {})
  };
}
