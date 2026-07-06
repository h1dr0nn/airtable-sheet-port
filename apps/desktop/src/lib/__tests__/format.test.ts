import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { formatAbsoluteTime, formatRelativeTime, formatValue } from "../format";

const NOW = new Date("2026-07-06T12:00:00.000Z");

function isoAgo(ms: number): string {
  return new Date(NOW.getTime() - ms).toISOString();
}

describe("formatRelativeTime", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("returns 'just now' under the 45 second threshold", () => {
    expect(formatRelativeTime(isoAgo(10_000))).toBe("just now");
    expect(formatRelativeTime(isoAgo(44_000))).toBe("just now");
  });

  it("formats minutes under an hour", () => {
    expect(formatRelativeTime(isoAgo(5 * 60_000))).toBe("5m ago");
  });

  it("formats hours under a day", () => {
    expect(formatRelativeTime(isoAgo(3 * 3_600_000))).toBe("3h ago");
  });

  it("formats days under a week", () => {
    expect(formatRelativeTime(isoAgo(2 * 86_400_000))).toBe("2d ago");
  });

  it("falls back to a locale date at a week or older", () => {
    const iso = isoAgo(8 * 86_400_000);
    expect(formatRelativeTime(iso)).toBe(new Date(iso).toLocaleDateString());
  });

  it("returns the input unchanged when it is not a parseable date", () => {
    expect(formatRelativeTime("not-a-date")).toBe("not-a-date");
  });
});

describe("formatAbsoluteTime", () => {
  it("renders a locale timestamp for a valid iso string", () => {
    const iso = "2026-07-06T08:30:00.000Z";
    expect(formatAbsoluteTime(iso)).toBe(new Date(iso).toLocaleString());
  });

  it("returns the input unchanged when it is not a parseable date", () => {
    expect(formatAbsoluteTime("garbage")).toBe("garbage");
  });
});

describe("formatValue", () => {
  it("renders null and undefined as an empty string", () => {
    expect(formatValue(null)).toBe("");
    expect(formatValue(undefined)).toBe("");
  });

  it("passes strings through unchanged", () => {
    expect(formatValue("Aurora Labs")).toBe("Aurora Labs");
  });

  it("stringifies numbers and booleans", () => {
    expect(formatValue(42)).toBe("42");
    expect(formatValue(false)).toBe("false");
  });

  it("JSON-encodes objects and arrays", () => {
    expect(formatValue({ a: 1 })).toBe('{"a":1}');
    expect(formatValue([1, 2])).toBe("[1,2]");
  });
});
