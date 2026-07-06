import { randomUUID } from "node:crypto";
import type { DatabaseSync } from "node:sqlite";
import type { FieldSchema, ReadTableOptions, RecordPatch, TableRecord, TableRef, TableSchema } from "@sheet-port/shared";
import { parseJsonColumn } from "../util.js";

export type MockRecordPage = {
  records: TableRecord[];
  total: number;
};

type MockTableRow = {
  source_id: string;
  table_id: string;
  name: string;
  fields: string;
};

type MockRecordRow = {
  record_id: string;
  fields: string;
};

/** SQLite: LIMIT -1 means "no limit". */
const NO_LIMIT = -1;

export class MockDataStore {
  constructor(private readonly db: DatabaseSync) {}

  listTables(sourceId: string): TableRef[] {
    const rows = this.db
      .prepare("SELECT source_id, table_id, name, fields FROM mock_tables WHERE source_id = ? ORDER BY table_id")
      .all(sourceId) as MockTableRow[];
    return rows.map((row) => ({ sourceId: row.source_id, tableId: row.table_id, name: row.name }));
  }

  getTable(sourceId: string, tableId: string): TableSchema | undefined {
    const row = this.db
      .prepare("SELECT source_id, table_id, name, fields FROM mock_tables WHERE source_id = ? AND table_id = ?")
      .get(sourceId, tableId) as MockTableRow | undefined;
    if (!row) {
      return undefined;
    }
    return {
      sourceId: row.source_id,
      tableId: row.table_id,
      name: row.name,
      fields: parseJsonColumn<FieldSchema[]>(row.fields)
    };
  }

  listRecords(sourceId: string, tableId: string, options: ReadTableOptions = {}): MockRecordPage {
    const totalRow = this.db
      .prepare("SELECT COUNT(*) AS total FROM mock_records WHERE source_id = ? AND table_id = ?")
      .get(sourceId, tableId) as { total: number };
    const rows = this.db
      .prepare(
        `SELECT record_id, fields FROM mock_records
         WHERE source_id = ? AND table_id = ?
         ORDER BY position LIMIT ? OFFSET ?`
      )
      .all(sourceId, tableId, options.limit ?? NO_LIMIT, options.offset ?? 0) as MockRecordRow[];
    return { records: rows.map(mapRecordRow), total: totalRow.total };
  }

  appendRecords(sourceId: string, tableId: string, records: Array<Record<string, unknown>>): TableRecord[] {
    return this.inTransaction(() => {
      const positionRow = this.db
        .prepare("SELECT COALESCE(MAX(position), 0) AS max_position FROM mock_records WHERE source_id = ? AND table_id = ?")
        .get(sourceId, tableId) as { max_position: number };
      const insert = this.db.prepare(
        "INSERT INTO mock_records (source_id, table_id, record_id, fields, position) VALUES (?, ?, ?, ?, ?)"
      );
      return records.map((fields, index) => {
        const record: TableRecord = { id: `rec_${randomUUID()}`, fields: { ...fields } };
        insert.run(sourceId, tableId, record.id, JSON.stringify(record.fields), positionRow.max_position + index + 1);
        return record;
      });
    });
  }

  /**
   * Shallow-merges patch fields into stored fields. Unknown record ids are
   * skipped (mirrors the previous in-memory connector semantics); only the
   * records that were actually updated are returned.
   */
  updateRecords(sourceId: string, tableId: string, patches: RecordPatch[]): TableRecord[] {
    return this.inTransaction(() => {
      const select = this.db.prepare(
        "SELECT record_id, fields FROM mock_records WHERE source_id = ? AND table_id = ? AND record_id = ?"
      );
      const update = this.db.prepare(
        "UPDATE mock_records SET fields = ? WHERE source_id = ? AND table_id = ? AND record_id = ?"
      );
      return patches.flatMap((patch) => {
        const row = select.get(sourceId, tableId, patch.recordId) as MockRecordRow | undefined;
        if (!row) {
          return [];
        }
        const merged = { ...parseJsonColumn<Record<string, unknown>>(row.fields), ...patch.fields };
        update.run(JSON.stringify(merged), sourceId, tableId, patch.recordId);
        return [{ id: patch.recordId, fields: merged }];
      });
    });
  }

  private inTransaction<T>(work: () => T): T {
    this.db.exec("BEGIN IMMEDIATE");
    try {
      const result = work();
      this.db.exec("COMMIT");
      return result;
    } catch (error) {
      this.db.exec("ROLLBACK");
      throw error;
    }
  }
}

function mapRecordRow(row: MockRecordRow): TableRecord {
  return { id: row.record_id, fields: parseJsonColumn<Record<string, unknown>>(row.fields) };
}
