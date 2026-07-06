import type { PermissionRule } from "@sheet-port/shared";
import { describe, expect, it } from "vitest";
import { ChangeService, ConnectorRegistry, PermissionDeniedError, PermissionService } from "../src/index.js";
import { FakeConnector, InMemoryChangeStore, InMemoryRuleProvider } from "./fakes.js";

const SOURCE = "src-a";
const TABLE = "t1";

const ALLOW_ALL_RULE: PermissionRule = {
  sourceId: SOURCE,
  read: true,
  write: true,
  deleteRecords: false,
  requireConfirmationFor: []
};

function setup() {
  const provider = new InMemoryRuleProvider();
  provider.set(ALLOW_ALL_RULE);
  const store = new InMemoryChangeStore();
  const connector = new FakeConnector("mock", [{ id: SOURCE, kind: "mock", name: "Fake Source" }]);
  connector.seed(SOURCE, TABLE, [{ id: "rec_1", fields: { Name: "Aurora", Seats: 2 } }]);
  const registry = new ConnectorRegistry((sourceId) => (sourceId === SOURCE ? "mock" : undefined));
  registry.register(connector);
  const service = new ChangeService(store, new PermissionService(provider), registry);
  return { provider, store, registry, service };
}

describe("ChangeService", () => {
  describe("previews", () => {
    it("stores an append change as pending with an after-only diff", () => {
      // Arrange
      const { store, service } = setup();

      // Act
      const change = service.createAppendChange(SOURCE, TABLE, [{ Name: "Delta" }], true);

      // Assert
      expect(change.status).toBe("pending");
      expect(change.type).toBe("append");
      expect(change.requiresConfirmation).toBe(true);
      expect(change.diff).toEqual({ after: [{ Name: "Delta" }] });
      expect(store.getPayload(change.id)).toEqual({ type: "append", records: [{ Name: "Delta" }] });
    });

    it("builds a before/after diff per record for updates", async () => {
      // Arrange
      const { service } = setup();

      // Act
      const change = await service.createUpdateChange(
        SOURCE,
        TABLE,
        [
          { recordId: "rec_1", fields: { Seats: 5 } },
          { recordId: "rec_missing", fields: { Name: "Ghost" } }
        ],
        false
      );

      // Assert: known records diff against current fields, unknown ones get before: null.
      expect(change.diff).toEqual([
        { recordId: "rec_1", before: { Name: "Aurora", Seats: 2 }, after: { Name: "Aurora", Seats: 5 } },
        { recordId: "rec_missing", before: null, after: { Name: "Ghost" } }
      ]);
    });
  });

  describe("commit enforcement", () => {
    it("blocks commit while a confirmation-required change is still pending", async () => {
      // Arrange
      const { store, service } = setup();
      const change = service.createAppendChange(SOURCE, TABLE, [{ Name: "Delta" }], true);

      // Act + Assert: the documented enforcement error from docs/ipc.md.
      await expect(service.commit(change.id)).rejects.toThrow(
        `Change ${change.id} requires user approval in the Airtable - Sheet Port desktop app before commit`
      );
      expect(store.get(change.id)?.status).toBe("pending");
    });

    it("commits a confirmation-required change after the desktop approves it", async () => {
      // Arrange
      const { store, registry, service } = setup();
      const change = await service.createUpdateChange(SOURCE, TABLE, [{ recordId: "rec_1", fields: { Seats: 5 } }], true);
      store.transition(change.id, "pending", "approved", "user");

      // Act
      const result = await service.commit(change.id);

      // Assert
      expect(result.change.status).toBe("committed");
      expect(result.change.decidedBy).toBe("user");
      expect(result.records).toEqual([{ id: "rec_1", fields: { Name: "Aurora", Seats: 5 } }]);
      const records = await registry.readTable(SOURCE, TABLE);
      expect(records[0]?.fields).toEqual({ Name: "Aurora", Seats: 5 });
    });

    it("auto-approves non-confirmation changes by policy at commit", async () => {
      // Arrange
      const { registry, service } = setup();
      const change = service.createAppendChange(SOURCE, TABLE, [{ Name: "Delta" }], false);

      // Act
      const result = await service.commit(change.id);

      // Assert
      expect(result.change.status).toBe("committed");
      expect(result.change.decidedBy).toBe("policy");
      expect(result.records).toHaveLength(1);
      const records = await registry.readTable(SOURCE, TABLE);
      expect(records).toHaveLength(2);
    });

    it("rejects committing a change rejected in the desktop app", async () => {
      // Arrange
      const { store, service } = setup();
      const change = service.createAppendChange(SOURCE, TABLE, [{ Name: "Delta" }], true);
      store.forceStatus(change.id, "rejected");

      // Act + Assert
      await expect(service.commit(change.id)).rejects.toThrow(
        `Change ${change.id} was rejected in the desktop app and cannot be committed`
      );
    });

    it("rejects a double commit of the same change", async () => {
      // Arrange
      const { service } = setup();
      const change = service.createAppendChange(SOURCE, TABLE, [{ Name: "Delta" }], false);
      await service.commit(change.id);

      // Act + Assert
      await expect(service.commit(change.id)).rejects.toThrow(`Change ${change.id} is already committed`);
    });

    it("rejects an unknown change id", async () => {
      const { service } = setup();
      await expect(service.commit("chg_nope")).rejects.toThrow("Unknown change chg_nope");
    });

    it("re-checks permissions at commit and fails when write was revoked after preview", async () => {
      // Arrange
      const { provider, store, service } = setup();
      const change = service.createAppendChange(SOURCE, TABLE, [{ Name: "Delta" }], false);
      provider.set({ ...ALLOW_ALL_RULE, write: false });

      // Act + Assert
      await expect(service.commit(change.id)).rejects.toThrow(PermissionDeniedError);
      expect(store.get(change.id)?.status).toBe("pending");
    });
  });

  describe("listing", () => {
    it("lists changes through the store, optionally filtered by status", async () => {
      // Arrange
      const { service } = setup();
      const committed = service.createAppendChange(SOURCE, TABLE, [{ Name: "A" }], false);
      await service.commit(committed.id);
      const pending = service.createAppendChange(SOURCE, TABLE, [{ Name: "B" }], true);

      // Act + Assert
      expect(service.list().map((change) => change.id).sort()).toEqual([committed.id, pending.id].sort());
      expect(service.list("pending").map((change) => change.id)).toEqual([pending.id]);
      expect(service.get(pending.id)?.id).toBe(pending.id);
    });
  });
});
