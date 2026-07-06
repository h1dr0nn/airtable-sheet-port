// Runtime guards for the change diff shapes documented in
// packages/storage/schema.sql (append -> {"after": records},
// update -> [{"recordId","before","after"}]).

export type AppendDiff = {
  after: Array<Record<string, unknown>>;
};

export type UpdateDiffEntry = {
  recordId: string;
  before: Record<string, unknown> | null;
  after: Record<string, unknown>;
};

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

export function parseAppendDiff(diff: unknown): AppendDiff | null {
  if (!isPlainObject(diff) || !Array.isArray(diff.after)) {
    return null;
  }
  const after = diff.after.filter(isPlainObject);
  return after.length === diff.after.length ? { after } : null;
}

export function parseUpdateDiff(diff: unknown): UpdateDiffEntry[] | null {
  if (!Array.isArray(diff)) {
    return null;
  }
  const entries: UpdateDiffEntry[] = [];
  for (const item of diff) {
    if (!isPlainObject(item) || typeof item.recordId !== "string" || !isPlainObject(item.after)) {
      return null;
    }
    const before = isPlainObject(item.before) ? item.before : null;
    entries.push({ recordId: item.recordId, before, after: item.after });
  }
  return entries;
}

/** True when a field value changed between before and after snapshots. */
export function isFieldChanged(entry: UpdateDiffEntry, field: string): boolean {
  if (entry.before === null) {
    return true;
  }
  return JSON.stringify(entry.before[field]) !== JSON.stringify(entry.after[field]);
}
