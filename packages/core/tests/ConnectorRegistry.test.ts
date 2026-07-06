import type { DataSourceKind } from "@sheet-port/shared";
import { describe, expect, it } from "vitest";
import { ConnectorRegistry } from "../src/index.js";
import { FakeConnector } from "./fakes.js";

const KIND_BY_SOURCE: Record<string, DataSourceKind> = {
  "src-mock": "mock",
  "src-google": "google_sheets",
  "src-provider": "provider"
};

function setup() {
  const mock = new FakeConnector("mock", [{ id: "src-mock", kind: "mock", name: "Mock" }]);
  mock.seed("src-mock", "customers", [{ id: "rec_m1", fields: { Name: "Mock Row" } }]);
  const google = new FakeConnector("google_sheets", [{ id: "src-google", kind: "google_sheets", name: "Sheets" }]);
  google.seed("src-google", "orders", [{ id: "rec_g1", fields: { Name: "Sheet Row" } }]);

  const registry = new ConnectorRegistry((sourceId) => KIND_BY_SOURCE[sourceId]);
  registry.register(mock);
  registry.register(google);
  return { registry };
}

describe("ConnectorRegistry", () => {
  it("routes calls to the connector matching the resolved source kind", async () => {
    // Arrange
    const { registry } = setup();

    // Act
    const mockTables = await registry.listTables("src-mock");
    const googleRecords = await registry.readTable("src-google", "orders");

    // Assert
    expect(mockTables).toEqual([{ sourceId: "src-mock", tableId: "customers", name: "customers" }]);
    expect(googleRecords).toEqual([{ id: "rec_g1", fields: { Name: "Sheet Row" } }]);
  });

  it("throws for an unknown source id", async () => {
    const { registry } = setup();
    await expect(registry.readTable("src-nope", "customers")).rejects.toThrow("Unknown source src-nope");
  });

  it("throws when no connector is registered for the resolved kind", async () => {
    const { registry } = setup();
    await expect(registry.listTables("src-provider")).rejects.toThrow(
      "No connector registered for source kind provider (source src-provider)"
    );
  });

  it("aggregates sources across all registered connectors", async () => {
    // Arrange
    const { registry } = setup();

    // Act
    const sources = await registry.listSources();

    // Assert
    expect(sources.map((source) => source.id).sort()).toEqual(["src-google", "src-mock"]);
  });

  it("routes writes through the resolved connector", async () => {
    // Arrange
    const { registry } = setup();

    // Act
    const appended = await registry.appendRecords("src-mock", "customers", [{ Name: "New" }]);
    const updated = await registry.updateRecords("src-mock", "customers", [
      { recordId: "rec_m1", fields: { Name: "Renamed" } }
    ]);
    const found = await registry.findRecords("src-mock", "customers", "renamed");

    // Assert
    expect(appended[0]?.id).toMatch(/^rec_/);
    expect(updated).toEqual([{ id: "rec_m1", fields: { Name: "Renamed" } }]);
    expect(found.map((record) => record.id)).toEqual(["rec_m1"]);
  });
});
