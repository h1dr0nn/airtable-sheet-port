import type { DatabaseSync } from "node:sqlite";
import type { PermissionRuleProvider } from "@sheet-port/core";
import type { ConfirmationAction, PermissionRule } from "@sheet-port/shared";
import { fromBool, nowIso, parseJsonColumn, toBool } from "../util.js";

export type StoredPermissionRule = PermissionRule & {
  id: number;
  updatedAt: string;
};

type PermissionRuleRow = {
  id: number;
  source_id: string;
  table_id: string | null;
  can_read: number;
  can_write: number;
  can_delete: number;
  require_confirmation: string;
  updated_at: string;
};

const RULE_COLUMNS =
  "id, source_id, table_id, can_read, can_write, can_delete, require_confirmation, updated_at";

export class PermissionStore implements PermissionRuleProvider {
  constructor(private readonly db: DatabaseSync) {}

  list(): StoredPermissionRule[] {
    const rows = this.db
      .prepare(`SELECT ${RULE_COLUMNS} FROM permission_rules ORDER BY source_id, table_id`)
      .all() as PermissionRuleRow[];
    return rows.map(mapRuleRow);
  }

  findRule(sourceId: string, tableId?: string): PermissionRule | undefined {
    return this.get(sourceId, tableId);
  }

  /** Table-specific rule wins over the source-wide (NULL table_id) rule. */
  get(sourceId: string, tableId?: string): StoredPermissionRule | undefined {
    const row =
      tableId === undefined
        ? (this.db
            .prepare(`SELECT ${RULE_COLUMNS} FROM permission_rules WHERE source_id = ? AND table_id IS NULL LIMIT 1`)
            .get(sourceId) as PermissionRuleRow | undefined)
        : (this.db
            .prepare(
              `SELECT ${RULE_COLUMNS} FROM permission_rules
               WHERE source_id = ? AND (table_id = ? OR table_id IS NULL)
               ORDER BY (table_id IS NULL) ASC LIMIT 1`
            )
            .get(sourceId, tableId) as PermissionRuleRow | undefined);
    return row ? mapRuleRow(row) : undefined;
  }

  upsert(rule: PermissionRule): StoredPermissionRule {
    // Explicit lookup instead of ON CONFLICT: SQLite treats NULL table_id
    // values as distinct in the UNIQUE(source_id, table_id) constraint.
    const existing = this.db
      .prepare("SELECT id FROM permission_rules WHERE source_id = ? AND table_id IS ?")
      .get(rule.sourceId, rule.tableId ?? null) as { id: number } | undefined;

    const id = existing ? this.updateRow(existing.id, rule) : this.insertRow(rule);
    const stored = this.getById(id);
    if (!stored) {
      throw new Error(`Permission rule ${id} not found after upsert`);
    }
    return stored;
  }

  delete(id: number): void {
    const result = this.db.prepare("DELETE FROM permission_rules WHERE id = ?").run(id);
    if (result.changes === 0) {
      throw new Error(`Unknown permission rule ${id}`);
    }
  }

  private getById(id: number): StoredPermissionRule | undefined {
    const row = this.db
      .prepare(`SELECT ${RULE_COLUMNS} FROM permission_rules WHERE id = ?`)
      .get(id) as PermissionRuleRow | undefined;
    return row ? mapRuleRow(row) : undefined;
  }

  private insertRow(rule: PermissionRule): number {
    const result = this.db
      .prepare(
        `INSERT INTO permission_rules
           (source_id, table_id, can_read, can_write, can_delete, require_confirmation, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)`
      )
      .run(
        rule.sourceId,
        rule.tableId ?? null,
        fromBool(rule.read),
        fromBool(rule.write),
        fromBool(rule.deleteRecords),
        JSON.stringify(rule.requireConfirmationFor),
        nowIso()
      );
    return Number(result.lastInsertRowid);
  }

  private updateRow(id: number, rule: PermissionRule): number {
    this.db
      .prepare(
        `UPDATE permission_rules
         SET can_read = ?, can_write = ?, can_delete = ?, require_confirmation = ?, updated_at = ?
         WHERE id = ?`
      )
      .run(
        fromBool(rule.read),
        fromBool(rule.write),
        fromBool(rule.deleteRecords),
        JSON.stringify(rule.requireConfirmationFor),
        nowIso(),
        id
      );
    return id;
  }
}

function mapRuleRow(row: PermissionRuleRow): StoredPermissionRule {
  return {
    id: row.id,
    sourceId: row.source_id,
    ...(row.table_id !== null ? { tableId: row.table_id } : {}),
    read: toBool(row.can_read),
    write: toBool(row.can_write),
    deleteRecords: toBool(row.can_delete),
    requireConfirmationFor: parseJsonColumn<ConfirmationAction[]>(row.require_confirmation),
    updatedAt: row.updated_at
  };
}
