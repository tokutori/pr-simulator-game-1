import { IDBDatabase as FakeDatabase, IDBFactory } from "fake-indexeddb";
import { describe, expect, it, vi } from "vitest";
import {
  IndexedDbFlightRecordPersistence,
  type PersonalBestSelectionPort
} from "../../web/src/game/flight-record-store.js";

function selection(): PersonalBestSelectionPort {
  return {
    is_eligible: () => false,
    candidate_is_best: () => false,
    key_hex: () => "",
    selected_existing_id: () => 0,
    consider_existing: () => undefined,
    free: () => undefined
  };
}

function observeOpen(factory: IDBFactory, onSuccess: (database: IDBDatabase) => void = () => undefined): {
  readonly requests: IDBOpenDBRequest[];
  readonly connections: IDBDatabase[];
} {
  const requests: IDBOpenDBRequest[] = [];
  const connections: IDBDatabase[] = [];
  const open = factory.open.bind(factory);
  vi.spyOn(factory, "open").mockImplementation((name, version) => {
    const request = open(name, version);
    request.addEventListener("success", () => {
      connections.push(request.result);
      onSuccess(request.result);
    });
    requests.push(request);
    return request;
  });
  return { requests, connections };
}

function latest<Value>(values: readonly Value[]): Value {
  const value = values.at(-1);
  if (value === undefined) throw new Error("Expected an observed IndexedDB resource");
  return value;
}

function openResult(request: IDBOpenDBRequest): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    request.addEventListener("success", () => { resolve(request.result); }, { once: true });
    request.addEventListener("error", () => { reject(request.error ?? new Error("Test database open failed")); }, { once: true });
  });
}

async function cleanup(factory: IDBFactory, name: string, connections: readonly IDBDatabase[]): Promise<void> {
  vi.restoreAllMocks();
  for (const connection of connections) connection.close();
  await new Promise<void>((resolve, reject) => {
    const request = factory.deleteDatabase(name);
    request.onsuccess = () => { resolve(); };
    request.onerror = () => { reject(request.error ?? new Error("Test database deletion failed")); };
    request.onblocked = () => { reject(new Error("Test database cleanup remained blocked")); };
  });
}

describe("IndexedDB connection ownership", () => {
  it.each(["get", "getAll", "add"] as const)("closes late success once after blocked %s and permits retry", async (operation) => {
    const factory = new IDBFactory();
    const name = `blocked-record-${operation}`;
    const observed = observeOpen(factory);
    const close = vi.spyOn(FakeDatabase.prototype, "close");
    const transaction = vi.spyOn(FakeDatabase.prototype, "transaction");
    const persistence = new IndexedDbFlightRecordPersistence(factory, name);
    const invoke = (): Promise<unknown> => {
      switch (operation) {
        case "get": return persistence.get(1);
        case "getAll": return persistence.getAll();
        case "add": return persistence.add("{}", "2026-10-03T00:00:00.000Z", selection());
      }
    };
    const events: string[] = [];
    try {
      const failed = invoke();
      const request = latest(observed.requests);
      request.addEventListener("upgradeneeded", () => { events.push("upgrade"); });
      request.addEventListener("success", () => { events.push("success"); });
      const lateSuccess = openResult(request);
      request.onblocked?.call(request, new Event("blocked") as IDBVersionChangeEvent);
      events.push("blocked");
      await expect(failed).rejects.toThrow("IndexedDB database upgrade is blocked");
      events.push("rejected");
      expect(observed.connections).toHaveLength(0);
      const abandoned = await lateSuccess;
      expect(events).toEqual(["blocked", "rejected", "upgrade", "success"]);
      expect(close.mock.contexts.filter((database) => database === abandoned)).toHaveLength(1);
      expect(transaction.mock.calls.filter(([, mode], index) =>
        transaction.mock.contexts[index] === abandoned && mode !== "versionchange"
      )).toHaveLength(0);
      expect(() => abandoned.transaction("records", "readonly")).toThrow();
      expect([...abandoned.objectStoreNames]).toEqual([
        "personalBests", "recordMetadata", "records"
      ]);
      await expect(invoke()).resolves.toEqual(operation === "get" ? null : operation === "getAll" ? [] : 1);
      const retried = latest(observed.connections);
      expect(retried).not.toBe(abandoned);
      expect(close.mock.contexts.filter((database) => database === retried)).toHaveLength(1);
      expect(close.mock.contexts.filter((database) => database === abandoned)).toHaveLength(1);
    } finally {
      await cleanup(factory, name, observed.connections);
    }
  });

  it("transfers a successful open to the operation and closes once on completion", async () => {
    const factory = new IDBFactory();
    const name = "normal-record-open";
    const observed = observeOpen(factory);
    const close = vi.spyOn(FakeDatabase.prototype, "close");
    const transaction = vi.spyOn(FakeDatabase.prototype, "transaction");
    try {
      await expect(new IndexedDbFlightRecordPersistence(factory, name).get(1)).resolves.toBeNull();
      const database = latest(observed.connections);
      expect(close.mock.contexts.filter((connection) => connection === database)).toHaveLength(1);
      expect(transaction.mock.calls.filter(([, mode], index) =>
        transaction.mock.contexts[index] === database && mode !== "versionchange"
      )).toHaveLength(1);
      expect(() => database.transaction("records", "readonly")).toThrow();
    } finally {
      await cleanup(factory, name, observed.connections);
    }
  });

  it("rejects a synchronous factory exception without acquiring a connection and permits retry", async () => {
    const factory = new IDBFactory();
    const name = "factory-throw-record";
    const failure = new Error("Injected synchronous open failure");
    const observed = observeOpen(factory);
    vi.spyOn(factory, "open").mockImplementationOnce(() => { throw failure; });
    const persistence = new IndexedDbFlightRecordPersistence(factory, name);
    try {
      await expect(persistence.get(1)).rejects.toBe(failure);
      expect(observed.requests).toHaveLength(0);
      expect(observed.connections).toHaveLength(0);
      await expect(persistence.get(1)).resolves.toBeNull();
    } finally {
      await cleanup(factory, name, observed.connections);
    }
  });

  it("rejects unsupported database versions and retains stored bytes", async () => {
    const factory = new IDBFactory();
    const name = "unsupported-record-database";
    const request = factory.open(name, 1);
    request.onupgradeneeded = () => {
      request.result.createObjectStore("records", { keyPath: "id" }).add({ id: 1, json: "retained bytes" });
    };
    const original = await openResult(request);
    original.close();
    const observed = observeOpen(factory);
    try {
      await expect(new IndexedDbFlightRecordPersistence(factory, name).get(1))
        .rejects.toThrow("Unsupported flight record database version; stored data is retained");
      const retained = await openResult(factory.open(name, 1));
      expect(retained.version).toBe(1);
      const bytes = await new Promise<unknown>((resolve, reject) => {
        const read = retained.transaction("records", "readonly").objectStore("records").get(1);
        read.onsuccess = () => { resolve(read.result); };
        read.onerror = () => { reject(read.error ?? new Error("Retained database read failed")); };
      });
      expect(bytes).toEqual({ id: 1, json: "retained bytes" });
      retained.close();
    } finally {
      await cleanup(factory, name, observed.connections);
    }
  });

  it("preserves native open errors without reading a nonexistent connection", async () => {
    const factory = new IDBFactory();
    const name = "version-error-record";
    const blocker = await openResult(factory.open(name, 5));
    blocker.close();
    const observed = observeOpen(factory);
    const close = vi.spyOn(FakeDatabase.prototype, "close");
    try {
      await expect(new IndexedDbFlightRecordPersistence(factory, name).get(1)).rejects.toMatchObject({ name: "VersionError" });
      expect(observed.connections).toHaveLength(0);
      expect(close).not.toHaveBeenCalled();
    } finally {
      await cleanup(factory, name, observed.connections);
    }
  });

  it.each(["synchronous throw", "asynchronous abort"] as const)("closes transferred connections after operation %s", async (failureMode) => {
    const factory = new IDBFactory();
    const name = `operation-failure-${failureMode}`;
    const failure = new Error("Injected transaction construction failure");
    let inject = true;
    const observed = observeOpen(factory, (database) => {
      if (!inject) return;
      inject = false;
      const transaction = database.transaction.bind(database);
      vi.spyOn(database, "transaction").mockImplementation((...args) => {
        if (failureMode === "synchronous throw") throw failure;
        const pending = transaction(...args);
        queueMicrotask(() => { pending.abort(); });
        return pending;
      });
    });
    const close = vi.spyOn(FakeDatabase.prototype, "close");
    const persistence = new IndexedDbFlightRecordPersistence(factory, name);
    try {
      if (failureMode === "synchronous throw") await expect(persistence.get(1)).rejects.toBe(failure);
      else await expect(persistence.get(1)).rejects.toMatchObject({ name: "AbortError" });
      const database = latest(observed.connections);
      expect(close.mock.contexts.filter((connection) => connection === database)).toHaveLength(1);
      await expect(persistence.get(1)).resolves.toBeNull();
      const retried = latest(observed.connections);
      expect(close.mock.contexts.filter((connection) => connection === retried)).toHaveLength(1);
    } finally {
      await cleanup(factory, name, observed.connections);
    }
  });
});
