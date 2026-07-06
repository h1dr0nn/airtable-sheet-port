export function nowIso(): string {
  return new Date().toISOString();
}

export function isoBefore(msAgo: number): string {
  return new Date(Date.now() - msAgo).toISOString();
}

/** Parses a JSON TEXT column; the caller vouches for the stored shape. */
export function parseJsonColumn<T>(text: string): T {
  return JSON.parse(text) as T;
}

export function toBool(value: number): boolean {
  return value !== 0;
}

export function fromBool(value: boolean): number {
  return value ? 1 : 0;
}
