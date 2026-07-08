import { describe, expect, it } from "vitest";
import {
  isFieldChanged,
  parseAppendDiff,
  parseFormatDiff,
  parseUpdateDiff,
  type UpdateDiffEntry
} from "../diff";

describe("parseAppendDiff", () => {
  it("parses a valid append diff", () => {
    // Arrange
    const diff: unknown = { after: [{ Name: "Aurora" }, { Name: "Basalt" }] };

    // Act
    const parsed = parseAppendDiff(diff);

    // Assert
    expect(parsed).toEqual({ after: [{ Name: "Aurora" }, { Name: "Basalt" }] });
  });

  it("returns null for non-object input", () => {
    expect(parseAppendDiff(null)).toBeNull();
    expect(parseAppendDiff("nope")).toBeNull();
    expect(parseAppendDiff([{ after: [] }])).toBeNull();
  });

  it("returns null when after is not an array", () => {
    expect(parseAppendDiff({ after: "records" })).toBeNull();
    expect(parseAppendDiff({})).toBeNull();
  });

  it("returns null when the after array contains non-record entries", () => {
    expect(parseAppendDiff({ after: [{ Name: "ok" }, 3] })).toBeNull();
    expect(parseAppendDiff({ after: [["not", "a", "record"]] })).toBeNull();
  });
});

describe("parseUpdateDiff", () => {
  it("parses valid entries and keeps a null before snapshot", () => {
    // Arrange
    const diff: unknown = [
      { recordId: "rec_1", before: { Seats: 24 }, after: { Seats: 25 } },
      { recordId: "rec_2", before: null, after: { Name: "New" } }
    ];

    // Act
    const parsed = parseUpdateDiff(diff);

    // Assert
    expect(parsed).toEqual([
      { recordId: "rec_1", before: { Seats: 24 }, after: { Seats: 25 } },
      { recordId: "rec_2", before: null, after: { Name: "New" } }
    ]);
  });

  it("coerces a non-record before value to null", () => {
    const parsed = parseUpdateDiff([{ recordId: "rec_1", before: "junk", after: { Seats: 1 } }]);
    expect(parsed).toEqual([{ recordId: "rec_1", before: null, after: { Seats: 1 } }]);
  });

  it("returns null for non-array input", () => {
    expect(parseUpdateDiff({ recordId: "rec_1" })).toBeNull();
    expect(parseUpdateDiff("nope")).toBeNull();
  });

  it("returns null when any entry is malformed", () => {
    expect(parseUpdateDiff([{ before: null, after: {} }])).toBeNull();
    expect(parseUpdateDiff([{ recordId: 7, before: null, after: {} }])).toBeNull();
    expect(parseUpdateDiff([{ recordId: "rec_1", before: null, after: "not-a-record" }])).toBeNull();
    expect(parseUpdateDiff([{ recordId: "rec_1", before: null, after: {} }, 42])).toBeNull();
  });
});

describe("parseFormatDiff", () => {
  it("parses a plan with cell formats, freeze, and column widths", () => {
    const diff: unknown = {
      formats: [{ range: "A1:D1", bold: true, backgroundColor: "#f3f4f6", border: "bottom" }],
      freezeRows: 1,
      columnWidths: [{ column: "A", pixels: 160 }]
    };

    const parsed = parseFormatDiff(diff);

    expect(parsed).toEqual({
      formats: [{ range: "A1:D1", bold: true, backgroundColor: "#f3f4f6", border: "bottom" }],
      freezeRows: 1,
      freezeColumns: undefined,
      columnWidths: [{ column: "A", pixels: 160 }]
    });
  });

  it("drops malformed formats and column widths", () => {
    const parsed = parseFormatDiff({
      formats: [{ range: "A1" }, { bold: true }],
      columnWidths: [{ column: "A", pixels: 100 }, { column: "B" }]
    });
    expect(parsed).toEqual({
      formats: [{ range: "A1" }],
      freezeRows: undefined,
      freezeColumns: undefined,
      columnWidths: [{ column: "A", pixels: 100 }]
    });
  });

  it("returns null when the plan carries no formatting", () => {
    expect(parseFormatDiff({})).toBeNull();
    expect(parseFormatDiff({ formats: [] })).toBeNull();
    expect(parseFormatDiff(null)).toBeNull();
    expect(parseFormatDiff([{ range: "A1" }])).toBeNull();
  });
});

describe("isFieldChanged", () => {
  function makeEntry(overrides: Partial<UpdateDiffEntry> = {}): UpdateDiffEntry {
    return { recordId: "rec_1", before: { Seats: 24, Plan: "pro" }, after: { Seats: 25, Plan: "pro" }, ...overrides };
  }

  it("treats every field as changed when before is null", () => {
    expect(isFieldChanged(makeEntry({ before: null }), "Plan")).toBe(true);
  });

  it("detects a changed value", () => {
    expect(isFieldChanged(makeEntry(), "Seats")).toBe(true);
  });

  it("detects an unchanged value", () => {
    expect(isFieldChanged(makeEntry(), "Plan")).toBe(false);
  });

  it("treats a newly added field as changed", () => {
    const entry = makeEntry({ before: { Seats: 24 }, after: { Seats: 24, Plan: "pro" } });
    expect(isFieldChanged(entry, "Plan")).toBe(true);
  });

  it("compares nested values structurally", () => {
    const entry = makeEntry({ before: { Tags: ["a", "b"] }, after: { Tags: ["a", "b"] } });
    expect(isFieldChanged(entry, "Tags")).toBe(false);
    const changed = makeEntry({ before: { Tags: ["a"] }, after: { Tags: ["a", "b"] } });
    expect(isFieldChanged(changed, "Tags")).toBe(true);
  });
});
