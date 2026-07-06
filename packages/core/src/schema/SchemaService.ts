import type { TableSchema } from "@sheet-port/shared";

export class SchemaService {
  private readonly schemas = new Map<string, TableSchema>();

  get(sourceId: string, tableId: string): TableSchema | undefined {
    return this.schemas.get(this.key(sourceId, tableId));
  }

  set(schema: TableSchema): void {
    this.schemas.set(this.key(schema.sourceId, schema.tableId), schema);
  }

  clearCache(): void {
    this.schemas.clear();
  }

  validateFields(schema: TableSchema, fields: Record<string, unknown>): string[] {
    const allowed = new Set(schema.fields.map((field) => field.name));
    return Object.keys(fields).filter((name) => !allowed.has(name));
  }

  private key(sourceId: string, tableId: string): string {
    return `${sourceId}:${tableId}`;
  }
}
