import type { DatabaseSync } from "node:sqlite";
import type { ChangeDecider, ChangePayload, ChangeStorePort } from "@sheet-port/core";
import type { ChangeStatus, ChangeType, PendingChange } from "@sheet-port/shared";
import { CHANGE_LIST_LIMIT } from "../constants.js";
import { fromBool, nowIso, parseJsonColumn, toBool } from "../util.js";

type ChangeRow = {
  id: string;
  source_id: string;
  table_id: string;
  change_type: string;
  created_at: string;
  status: string;
  requires_confirmation: number;
  diff: string;
  decided_at: string | null;
  decided_by: string | null;
  committed_at: string | null;
};

// payload is intentionally excluded: change rows returned to callers must never expose it.
const CHANGE_COLUMNS =
  "id, source_id, table_id, change_type, created_at, status, requires_confirmation, diff, decided_at, decided_by, committed_at";

export class ChangeStore implements ChangeStorePort {
  constructor(private readonly db: DatabaseSync) {}

  insert(change: PendingChange, payload: ChangePayload): void {
    this.db
      .prepare(
        `INSERT INTO pending_changes
           (id, source_id, table_id, change_type, created_at, status, requires_confirmation, diff, payload)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`
      )
      .run(
        change.id,
        change.sourceId,
        change.tableId,
        change.type,
        change.createdAt,
        change.status,
        fromBool(change.requiresConfirmation),
        JSON.stringify(change.diff ?? null),
        JSON.stringify(payload)
      );
  }

  get(changeId: string): PendingChange | undefined {
    const row = this.db
      .prepare(`SELECT ${CHANGE_COLUMNS} FROM pending_changes WHERE id = ?`)
      .get(changeId) as ChangeRow | undefined;
    return row ? mapChangeRow(row) : undefined;
  }

  getPayload(changeId: string): ChangePayload | undefined {
    const row = this.db.prepare("SELECT payload FROM pending_changes WHERE id = ?").get(changeId) as
      | { payload: string }
      | undefined;
    return row ? parseJsonColumn<ChangePayload>(row.payload) : undefined;
  }

  list(status?: ChangeStatus): PendingChange[] {
    const rows = (
      status === undefined
        ? this.db
            .prepare(`SELECT ${CHANGE_COLUMNS} FROM pending_changes ORDER BY created_at DESC LIMIT ?`)
            .all(CHANGE_LIST_LIMIT)
        : this.db
            .prepare(`SELECT ${CHANGE_COLUMNS} FROM pending_changes WHERE status = ? ORDER BY created_at DESC LIMIT ?`)
            .all(status, CHANGE_LIST_LIMIT)
    ) as ChangeRow[];
    return rows.map(mapChangeRow);
  }

  transition(changeId: string, from: ChangeStatus, to: ChangeStatus, decidedBy: ChangeDecider): boolean {
    const result = this.db
      .prepare("UPDATE pending_changes SET status = ?, decided_at = ?, decided_by = ? WHERE id = ? AND status = ?")
      .run(to, nowIso(), decidedBy, changeId, from);
    return result.changes === 1;
  }

  markCommitted(changeId: string): boolean {
    const result = this.db
      .prepare("UPDATE pending_changes SET status = 'committed', committed_at = ? WHERE id = ? AND status = 'approved'")
      .run(nowIso(), changeId);
    return result.changes === 1;
  }
}

function mapChangeRow(row: ChangeRow): PendingChange {
  return {
    id: row.id,
    sourceId: row.source_id,
    tableId: row.table_id,
    type: row.change_type as ChangeType,
    createdAt: row.created_at,
    status: row.status as ChangeStatus,
    requiresConfirmation: toBool(row.requires_confirmation),
    diff: parseJsonColumn<unknown>(row.diff),
    ...(row.decided_at !== null ? { decidedAt: row.decided_at } : {}),
    ...(row.decided_by !== null ? { decidedBy: row.decided_by as ChangeDecider } : {}),
    ...(row.committed_at !== null ? { committedAt: row.committed_at } : {})
  };
}
