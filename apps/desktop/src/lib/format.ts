const SECOND_MS = 1_000;
const MINUTE_MS = 60 * SECOND_MS;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;
const JUST_NOW_THRESHOLD_MS = 45 * SECOND_MS;
const RELATIVE_DAYS_LIMIT = 7;

/** Compact relative timestamp: "just now", "5m ago", "3h ago", "2d ago", then a date. */
export function formatRelativeTime(iso: string): string {
  const elapsed = Date.now() - new Date(iso).getTime();
  if (Number.isNaN(elapsed)) {
    return iso;
  }
  if (elapsed < JUST_NOW_THRESHOLD_MS) {
    return "just now";
  }
  if (elapsed < HOUR_MS) {
    return `${Math.round(elapsed / MINUTE_MS)}m ago`;
  }
  if (elapsed < DAY_MS) {
    return `${Math.round(elapsed / HOUR_MS)}h ago`;
  }
  if (elapsed < RELATIVE_DAYS_LIMIT * DAY_MS) {
    return `${Math.round(elapsed / DAY_MS)}d ago`;
  }
  return new Date(iso).toLocaleDateString();
}

export function formatAbsoluteTime(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime()) ? iso : date.toLocaleString();
}

/** Render an unknown cell/diff value as short display text. */
export function formatValue(value: unknown): string {
  if (value === null || value === undefined) {
    return "";
  }
  if (typeof value === "string") {
    return value;
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  return JSON.stringify(value);
}
