import { IDBFactory, IDBObjectStore } from "fake-indexeddb";
import { describe, expect, it, vi } from "vitest";
import { BrowserFlightRecordStore } from "../../web/src/game/flight-record-store.js";
import type { ApplicationInput, ApplicationRequest } from "../../web/src/generated/application-contracts.js";

function result<Value>(request: IDBRequest<Value>): Promise<Value> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => { resolve(request.result); };
    request.onerror = () => { reject(request.error ?? new Error("IndexedDB request failed")); };
  });
}

async function databaseFixture() {
  const factory = new IDBFactory();
  const name = "primitive-record-io";
  const request = factory.open(name, 4);
  request.onupgradeneeded = () => {
    request.result.createObjectStore("records", { keyPath: "id", autoIncrement: true });
    request.result.createObjectStore("recordMetadata", { keyPath: "id" });
    request.result.createObjectStore("personalBests", { keyPath: "key" });
  };
  const database = await result(request);
  return { factory, name, database };
}

describe("Browser record primitive I/O", () => {
  it("enqueues owner requests synchronously in transaction callbacks and retains exact stored bytes", async () => {
    const factory = new IDBFactory();
    const json = " {\n  \"source\": \"原文\", \"samples\": []\n}\n";
    const savedAt = "2026-10-23T00:00:00.000Z";
    const key = "a".repeat(64);
    const observations: ApplicationInput[] = [];
    let complete: (() => void) | undefined;
    let fail: ((cause: unknown) => void) | undefined;
    const done = new Promise<void>((resolve, reject) => { complete = resolve; fail = reject; });
    const store = new BrowserFlightRecordStore(factory, input => {
      observations.push(input);
      if (input.kind === "record_save_opened") {
        expect(Number.isFinite(input.unix_milliseconds)).toBe(true);
        store.execute({ kind: "record_write", request_id: input.request_id, requests: [{ kind: "insert_record", json, saved_at: savedAt }] });
      }
      if (input.kind !== "record_save_observed") return;
      const observation = input.observation;
      const requests: Extract<ApplicationRequest, { kind: "record_write" }>["requests"] = observation.kind === "inserted"
        ? [{ kind: "write_metadata", id: 1, saved_at: savedAt }]
        : observation.kind === "metadata_written"
          ? [{ kind: "write_best", entry: { key, recordId: 1, comparisonVersion: 1 } }]
          : observation.kind === "best_written" ? [{ kind: "finish" }] : [];
      if (observation.kind === "inserted") expect(observation.key).toBe(1);
      if (requests.length > 0) store.execute({ kind: "record_write", request_id: input.request_id, requests });
      if (observation.kind === "committed") complete?.();
      if (observation.kind === "failed") fail?.(new Error(observation.cause));
    });
    try {
      store.execute({ kind: "begin_record_save", request_id: 7 });
      await done;
      expect(observations.map(input => input.kind === "record_save_observed" ? input.observation.kind : input.kind))
        .toEqual(["record_save_opened", "inserted", "metadata_written", "best_written", "committed"]);
      const database = await result(factory.open("birdman-flight-records", 4));
      try {
        const transaction = database.transaction(["records", "recordMetadata", "personalBests"], "readonly");
        const record = result(transaction.objectStore("records").get(1));
        const metadata = result(transaction.objectStore("recordMetadata").get(1));
        const best = result(transaction.objectStore("personalBests").get(key));
        expect(await record).toEqual({ id: 1, savedAt, json });
        expect(await metadata).toEqual({ id: 1, savedAt });
        expect(await best).toEqual({ key, recordId: 1, comparisonVersion: 1 });
      } finally { database.close(); }
    } finally { store.dispose(); }
  });

  it("forwards bounded descending rows and raw index/record values without interpreting schemas", async () => {
    const fixture = await databaseFixture();
    const transaction = fixture.database.transaction(["records", "recordMetadata", "personalBests"], "readwrite");
    transaction.objectStore("records").put({ id: 1, json: "old schema bytes", savedAt: "first" });
    transaction.objectStore("records").put({ id: 2, unexpected: [null, false, 3] });
    transaction.objectStore("recordMetadata").put({ id: 1, savedAt: "first" });
    transaction.objectStore("recordMetadata").put({ id: 2, invalid: true });
    transaction.objectStore("personalBests").put({ key: "raw-key", unexpected: "retained" });
    await new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => { resolve(); };
      transaction.onabort = () => { reject(transaction.error ?? new Error("IndexedDB transaction aborted")); };
    });
    fixture.database.close();
    const inputs: ApplicationInput[] = [];
    let finish: (() => void) | undefined;
    const completed = new Promise<void>(resolve => { finish = resolve; });
    const store = new BrowserFlightRecordStore(fixture.factory, input => {
      inputs.push(input);
      if (input.kind !== "record_preview_observed") return;
      if (input.observation.kind === "rows") store.execute({ kind: "record_preview", request_id: input.request_id, requests: [{ kind: "read_best", keys: ["raw-key", "missing"] }] });
      if (input.observation.kind === "bests") store.execute({ kind: "record_preview", request_id: input.request_id, requests: [{ kind: "finish" }] });
      if (input.observation.kind === "committed" || input.observation.kind === "failed") finish?.();
    }, fixture.name);
    try {
      store.execute({ kind: "begin_record_preview", request_id: 8, limit: 1 });
      await completed;
      expect(inputs).toEqual([
        { kind: "record_preview_observed", request_id: 8, observation: { kind: "rows", rows: [{ key: 2, metadata: { id: 2, invalid: true }, record: { id: 2, unexpected: [null, false, 3] } }] } },
        { kind: "record_preview_observed", request_id: 8, observation: { kind: "bests", values: [{ key: "raw-key", unexpected: "retained" }, null] } },
        { kind: "record_preview_observed", request_id: 8, observation: { kind: "committed" } }
      ]);
    } finally { store.dispose(); }
  });

  it("reports transaction aborts to the owner and releases an open transaction exactly once", async () => {
    const inputs: ApplicationInput[] = [];
    let aborted: (() => void) | undefined;
    const done = new Promise<void>(resolve => { aborted = resolve; });
    const store = new BrowserFlightRecordStore(new IDBFactory(), input => {
      inputs.push(input);
      if (input.kind === "record_save_opened") {
        store.execute({ kind: "record_write", request_id: input.request_id, requests: [{ kind: "abort" }] });
      }
      if (input.kind === "record_save_observed" && input.observation.kind === "failed") aborted?.();
    });
    store.execute({ kind: "begin_record_save", request_id: 3 });
    await done;
    expect(inputs).toHaveLength(2);
    const failure = inputs[1];
    expect(failure).toMatchObject({ kind: "record_save_observed", request_id: 3, observation: { kind: "failed" } });
    if (failure?.kind !== "record_save_observed" || failure.observation.kind !== "failed") throw new Error("Missing transaction abort observation");
    expect(failure.observation.cause).toContain("aborted");
    store.dispose();
    store.dispose();
    expect(() => { store.execute({ kind: "read_record", request_id: 4, record_id: 1 }); }).toThrow("disposed");
  });

  it.each(["opening", "transaction"] as const)("acknowledges cancellation after the real %s I/O reaches its terminal event", async phase => {
    const inputs: ApplicationInput[] = [];
    let finish: (() => void) | undefined;
    const done = new Promise<void>(resolve => { finish = resolve; });
    const store = new BrowserFlightRecordStore(new IDBFactory(), input => {
      inputs.push(input);
      if (input.kind === "record_save_opened") store.cancel(input.request_id);
      if (input.kind === "record_io_cancelled") finish?.();
    });
    try {
      store.execute({ kind: "begin_record_save", request_id: 12 });
      if (phase === "opening") {
        store.cancel(12);
        expect(inputs).toEqual([]);
      }
      await done;
      expect(inputs.map(input => input.kind)).toEqual(phase === "opening" ? ["record_io_cancelled"] : ["record_save_opened", "record_io_cancelled"]);
      expect(inputs.at(-1)).toEqual({ kind: "record_io_cancelled", request_id: 12, failure: null });
    } finally { store.dispose(); }
  });

  it.each([false, true])("notifies a single read only after transaction termination, cancelled=%s", async cancelled => {
    const inputs: ApplicationInput[] = [];
    let finish: (() => void) | undefined;
    const done = new Promise<void>(resolve => { finish = resolve; });
    let enqueue: (() => unknown) | undefined;
    const store = new BrowserFlightRecordStore(new IDBFactory(), input => {
      inputs.push(input);
      expect(enqueue).toBeDefined();
      expect(() => { enqueue?.(); }).toThrow();
      finish?.();
    });
    const original = Reflect.get(IDBObjectStore.prototype, "get");
    const read = vi.spyOn(IDBObjectStore.prototype, "get").mockImplementation(function (this: IDBObjectStore, query) {
      enqueue = () => original.call(this, query);
      const request = original.call(this, query);
      if (cancelled) request.addEventListener("success", () => { store.cancel(18); });
      return request;
    });
    try {
      store.execute({ kind: "read_record", request_id: 18, record_id: 1 });
      await done;
      expect(inputs).toEqual(cancelled
        ? [{ kind: "record_io_cancelled", request_id: 18, failure: null }]
        : [{ kind: "record_loaded", request_id: 18, completion: { kind: "missing" } }]);
    } finally { read.mockRestore(); store.dispose(); }
  });
});
