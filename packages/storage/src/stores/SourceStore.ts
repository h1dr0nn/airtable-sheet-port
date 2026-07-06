import type { DatabaseSync } from "node:sqlite";
import type { DataSource, DataSourceKind, SourceStatus } from "@sheet-port/shared";

type SourceRow = {
  id: string;
  kind: string;
  name: string;
  status: string;
};

export class SourceStore {
  constructor(private readonly db: DatabaseSync) {}

  list(): DataSource[] {
    const rows = this.db.prepare("SELECT id, kind, name, status FROM sources ORDER BY rowid").all() as SourceRow[];
    return rows.map(mapSourceRow);
  }

  getKind(sourceId: string): DataSourceKind | undefined {
    const row = this.db.prepare("SELECT kind FROM sources WHERE id = ?").get(sourceId) as
      | { kind: string }
      | undefined;
    return row ? (row.kind as DataSourceKind) : undefined;
  }
}

function mapSourceRow(row: SourceRow): DataSource {
  return {
    id: row.id,
    kind: row.kind as DataSourceKind,
    name: row.name,
    status: row.status as SourceStatus
  };
}
