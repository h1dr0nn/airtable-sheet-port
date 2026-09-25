import { beforeEach, describe, expect, it } from "vitest";
import { appToast, MAX_TOASTS, useToastStore, type ToastItem } from "@sheet-port/ui";

function item(id: string, overrides: Partial<ToastItem> = {}): ToastItem {
  return {
    id,
    title: `Title ${id}`,
    variant: "default",
    copyable: false,
    duration: 4000,
    ...overrides
  };
}

const ids = () => useToastStore.getState().toasts.map((t) => t.id);

describe("toast store", () => {
  beforeEach(() => {
    useToastStore.setState({ toasts: [] });
  });

  it("appends newest last and caps the stack at MAX_TOASTS", () => {
    const { add } = useToastStore.getState();
    for (let i = 1; i <= MAX_TOASTS + 2; i += 1) {
      add(item(`t${i}`));
    }
    expect(ids()).toEqual(["t3", "t4", "t5", "t6"]);
  });

  it("replaces a same-id toast in place without moving it", () => {
    const { add } = useToastStore.getState();
    add(item("a", { variant: "loading", duration: Infinity }));
    add(item("b"));
    const before = useToastStore.getState().toasts[0];

    const returned = add(item("a", { variant: "success", title: "Done" }));

    const toasts = useToastStore.getState().toasts;
    expect(returned).toBe("a");
    expect(ids()).toEqual(["a", "b"]);
    expect(toasts[0]).toMatchObject({ variant: "success", title: "Done", duration: 4000 });
    // A new object, so the card restarts its auto-dismiss timer.
    expect(toasts[0]).not.toBe(before);
  });

  it("refreshes an identical toast instead of stacking a copy when deduping", () => {
    const { add } = useToastStore.getState();
    add(item("a", { title: "Saved" }));
    add(item("b"));

    const returned = add(item("c", { title: "Saved" }), { dedupe: true });

    expect(returned).toBe("a");
    expect(ids()).toEqual(["a", "b"]);
  });

  it("stacks toasts that differ in variant or description", () => {
    const { add } = useToastStore.getState();
    add(item("a", { title: "Saved" }));
    add(item("b", { title: "Saved", variant: "error" }), { dedupe: true });
    add(item("c", { title: "Saved", description: "detail" }), { dedupe: true });
    expect(ids()).toEqual(["a", "b", "c"]);
  });

  it("removes one toast by id, or all without an id", () => {
    const { add, remove } = useToastStore.getState();
    add(item("a"));
    add(item("b"));
    remove("a");
    expect(ids()).toEqual(["b"]);
    remove();
    expect(ids()).toEqual([]);
  });
});

describe("appToast", () => {
  beforeEach(() => {
    useToastStore.setState({ toasts: [] });
  });

  it("dedupes repeated id-less toasts and returns the visible id", () => {
    const first = appToast({ title: "Copied", variant: "success" });
    const second = appToast({ title: "Copied", variant: "success" });
    expect(second).toBe(first);
    expect(ids()).toEqual([first]);
  });

  it("updates an explicit-id loading toast to its result in place", () => {
    appToast({ id: 7, title: "Exporting", variant: "loading" });
    appToast({ title: "Other" });
    appToast({ id: 7, title: "Exported", variant: "success" });

    const toasts = useToastStore.getState().toasts;
    expect(toasts.map((t) => t.id)[0]).toBe("7");
    expect(toasts[0]).toMatchObject({ title: "Exported", variant: "success", duration: 4000 });
  });
});
