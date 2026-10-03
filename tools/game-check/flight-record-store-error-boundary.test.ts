import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { IDBFactory, IDBObjectStore as FakeObjectStore } from "fake-indexeddb";
import { afterEach, describe, expect, it, vi, type MockInstance } from "vitest";
import { GameSessionBridge, PersonalBestSelectionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import {
  FlightRecordRepository,
  IndexedDbFlightRecordPersistence,
  type PersonalBestSelectionPort
} from "../../web/src/game/flight-record-store.js";

const storeNames = ["records", "recordMetadata", "personalBests", "personalBestIndexState"];
const savedAt = "2026-10-03T00:00:00.000Z";
const formatMessage = "flight record format error: UnsupportedSchemaVersion";
type ComparisonRoute = "repair" | "indexed" | "scan";

afterEach(() => { vi.restoreAllMocks(); });

let validRecord: string | undefined;

function waterContactRecord(): string {
  if (validRecord !== undefined) return validRecord;
  initSync({ module: new Uint8Array(readFileSync(fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url)))) });
  const session = new GameSessionBridge(0);
  try {
    session.open_setup();
    session.prepare();
    session.mark_briefing_ready();
    session.start_countdown(1);
    session.advance_countdown();
    let snapshot = parseFlightSnapshot(session.launch());
    for (let tick = 0; tick < 3_000 && snapshot.terminal === "airborne"; tick++) {
      snapshot = parseFlightSnapshot(session.advance_tick(0, 0, 0, 0));
    }
    expect(snapshot.terminal).toBe("water-contact");
    validRecord = session.export_flight_record_json();
    return validRecord;
  } finally {
    session.free();
  }
}

function requestResult<Value>(request: IDBRequest<Value>): Promise<Value> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => { resolve(request.result); };
    request.onerror = () => { reject(request.error ?? new Error("Test IndexedDB request failed")); };
  });
}

function completed(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => { resolve(); };
    transaction.onabort = () => { reject(transaction.error ?? new Error("Test IndexedDB transaction aborted")); };
  });
}

async function createFixture(route: ComparisonRoute, existingJson: string) {
  const factory = new IDBFactory();
  const name = `record-error-${route}`;
  const open = factory.open.bind(factory);
  const request = open(name, 4);
  request.onupgradeneeded = () => {
    const database = request.result;
    database.createObjectStore("records", { keyPath: "id", autoIncrement: true });
    database.createObjectStore("recordMetadata", { keyPath: "id" });
    database.createObjectStore("personalBests", { keyPath: "key" });
    database.createObjectStore("personalBestIndexState", { keyPath: "key" });
  };
  const database = await requestResult(request);
  const json = waterContactRecord();
  const selection = new PersonalBestSelectionBridge(json);
  let key: string;
  try { key = selection.key_hex(); } finally { selection.free(); }
  try {
    const transaction = database.transaction(storeNames, "readwrite");
    const done = completed(transaction);
    for (const [id, recordJson] of [[1, json], [2, existingJson]] as const) {
      transaction.objectStore("records").add({ id, json: recordJson, savedAt });
      transaction.objectStore("recordMetadata").add({ id, savedAt });
    }
    if (route !== "scan") transaction.objectStore("personalBests").put({ key, recordId: 2 });
    transaction.objectStore("personalBestIndexState").put({ key: route === "repair" ? "canonical-v1" : "first-winner-physics-v2" });
    await done;
  } finally {
    database.close();
  }
  const connections: { readonly database: IDBDatabase; readonly close: MockInstance<() => void> }[] = [];
  vi.spyOn(factory, "open").mockImplementation((...args) => {
    const pending = open(...args);
    pending.addEventListener("success", () => {
      connections.push({ database: pending.result, close: vi.spyOn(pending.result, "close") });
    });
    return pending;
  });
  return {
    factory, name, connections, json,
    async state(): Promise<Readonly<Record<string, unknown[]>>> {
      const connection = await requestResult(open(name));
      try {
        const transaction = connection.transaction(storeNames, "readonly");
        const done = completed(transaction);
        const entries = await Promise.all(storeNames.map(async (storeName) => [
          storeName, await requestResult(transaction.objectStore(storeName).getAll()) as unknown[]
        ] as const));
        await done;
        return Object.fromEntries(entries);
      } finally { connection.close(); }
    },
    async replaceExisting(): Promise<void> {
      const connection = await requestResult(open(name));
      try {
        const transaction = connection.transaction("records", "readwrite");
        const done = completed(transaction);
        transaction.objectStore("records").put({ id: 2, json, savedAt });
        await done;
      } finally { connection.close(); }
    },
    async dispose(): Promise<void> {
      for (const connection of connections) connection.database.close();
      await requestResult(factory.deleteDatabase(name));
    }
  };
}

function tracedSelections(injected?: { readonly reason: unknown }) {
  const entries: { freeCount: number }[] = [];
  const failures: unknown[] = [];
  let inject = injected !== undefined;
  return {
    entries, failures,
    disableInjection: () => { inject = false; },
    create: (json: string): PersonalBestSelectionPort => {
      let selection: PersonalBestSelectionBridge;
      try { selection = new PersonalBestSelectionBridge(json); }
      catch (error: unknown) { failures.push(error); throw error; }
      const entry = { freeCount: 0 };
      entries.push(entry);
      return {
        is_eligible: () => selection.is_eligible(),
        key_hex: () => selection.key_hex(),
        candidate_is_best: () => selection.candidate_is_best(),
        selected_existing_id: () => selection.selected_existing_id(),
        consider_existing: (id, recordJson) => {
          if (inject) throw injected?.reason;
          try { selection.consider_existing(id, recordJson); }
          catch (error: unknown) { failures.push(error); throw error; }
        },
        free: () => { entry.freeCount++; selection.free(); }
      };
    }
  };
}

async function rejection(operation: Promise<unknown>): Promise<unknown> {
  try { await operation; }
  catch (error: unknown) { return error; }
  throw new Error("Expected a rejected storage operation");
}

describe("FlightRecord storage error boundary", () => {
  it.each(["repair", "indexed", "scan"] as const)("preserves actual Rust format failure through %s and releases rolled-back resources", async (route) => {
    const document = JSON.parse(waterContactRecord()) as Record<string, unknown>;
    const fixture = await createFixture(route, JSON.stringify({ ...document, schema_version: 6 }));
    const trace = tracedSelections();
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(fixture.factory, fixture.name), () => new Date(savedAt), trace.create);
    try {
      const before = await fixture.state();
      const failure = await rejection(repository.saveFrom({ export_flight_record_json: () => fixture.json }));
      expect(trace.failures).toEqual([formatMessage]);
      expect(failure).toBeInstanceOf(Error);
      expect(failure).toMatchObject({ message: formatMessage, cause: formatMessage });
      expect(await fixture.state()).toEqual(before);
      expect(trace.entries.length).toBe(route === "repair" ? 2 : 1);
      expect(trace.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
      expect(fixture.connections).toHaveLength(1);
      expect(fixture.connections[0]?.close).toHaveBeenCalledTimes(1);
      expect(() => fixture.connections[0]?.database.transaction("records")).toThrow();
      await fixture.replaceExisting();
      const saved = await repository.saveFrom({ export_flight_record_json: () => fixture.json });
      expect(saved.id).toBe(3);
      expect(saved.personalBest).toMatchObject({ kind: "existing", id: route === "indexed" ? 2 : 1 });
      expect(await repository.load(2)).toBe(fixture.json);
      expect(await repository.list()).toHaveLength(3);
      expect(trace.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
      expect(fixture.connections.every(({ close }) => close.mock.calls.length === 1)).toBe(true);
    } finally { await fixture.dispose(); }
  });

  const errorObject = new Error("Existing selector failure");
  const unprintable = { [Symbol.toPrimitive]: () => { throw new Error("Coercion failed"); } };
  const revoked = Proxy.revocable({}, {});
  revoked.revoke();
  it.each([
    { name: "Error identity", reason: errorObject, message: errorObject.message },
    { name: "string", reason: "Selector primitive failure", message: "Selector primitive failure" },
    { name: "undefined", reason: undefined, message: "undefined" },
    { name: "null", reason: null, message: "null" },
    { name: "false", reason: false, message: "false" },
    { name: "zero", reason: 0, message: "0" },
    { name: "throwing coercion", reason: unprintable, message: "IndexedDB record insertion failed with an unprintable cause" },
    { name: "revoked Proxy", reason: revoked.proxy, message: "IndexedDB record insertion failed with an unprintable cause" }
  ])("normalizes $name without bypassing abort or resource cleanup", async ({ reason, message }) => {
    const fixture = await createFixture("scan", waterContactRecord());
    const trace = tracedSelections({ reason });
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(fixture.factory, fixture.name), () => new Date(savedAt), trace.create);
    try {
      const before = await fixture.state();
      const failure = await rejection(repository.saveFrom({ export_flight_record_json: () => fixture.json }));
      expect(failure).toBeInstanceOf(Error);
      expect((failure as Error).message).toBe(message);
      if (reason === errorObject) expect(failure).toBe(errorObject);
      else expect((failure as Error).cause).toBe(reason);
      expect(await fixture.state()).toEqual(before);
      expect(trace.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
      expect(fixture.connections[0]?.close).toHaveBeenCalledTimes(1);
      trace.disableInjection();
      await expect(repository.saveFrom({ export_flight_record_json: () => fixture.json })).resolves.toMatchObject({ id: 3 });
      expect(trace.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
      expect(fixture.connections.every(({ close }) => close.mock.calls.length === 1)).toBe(true);
    } finally { await fixture.dispose(); }
  });

  it.each(["request error", "explicit abort"] as const)("preserves the IndexedDB fallback for an unrecorded %s", async (mode) => {
    const fixture = await createFixture("scan", waterContactRecord());
    const trace = tracedSelections();
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(fixture.factory, fixture.name), () => new Date(savedAt), trace.create);
    const originalAdd = Object.getOwnPropertyDescriptor(FakeObjectStore.prototype, "add")?.value as IDBObjectStore["add"];
    let requestFailure: DOMException | null = null;
    let transactionFailure: DOMException | null = null;
    const add = vi.spyOn(FakeObjectStore.prototype, "add").mockImplementation(function (this: IDBObjectStore, value: unknown, key?: IDBValidKey) {
      const request = originalAdd.call(this, this.name === "records" && mode === "request error"
        ? { ...value as Record<string, unknown>, id: 1 }
        : value, key);
      if (this.name === "records") {
        if (mode === "request error") {
          request.addEventListener("error", () => { requestFailure = request.error; });
          this.transaction.addEventListener("abort", () => { transactionFailure = this.transaction.error; });
        }
        else queueMicrotask(() => { this.transaction.abort(); });
      }
      return request;
    });
    try {
      const before = await fixture.state();
      const failure = await rejection(repository.saveFrom({ export_flight_record_json: () => fixture.json }));
      add.mockRestore();
      if (mode === "request error") {
        expect(failure).toBe(transactionFailure);
        expect(requestFailure).toMatchObject({ name: "ConstraintError" });
        expect(failure).toMatchObject({ name: "ConstraintError" });
      } else {
        expect(failure).toBeInstanceOf(Error);
        expect(failure).toMatchObject({ message: "IndexedDB record insertion was aborted" });
        expect((failure as Error).cause).toBeUndefined();
      }
      expect(await fixture.state()).toEqual(before);
      expect(trace.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
      expect(fixture.connections[0]?.close).toHaveBeenCalledTimes(1);
      await expect(repository.saveFrom({ export_flight_record_json: () => fixture.json })).resolves.toMatchObject({ id: 3 });
      expect(trace.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
      expect(fixture.connections.every(({ close }) => close.mock.calls.length === 1)).toBe(true);
    } finally { add.mockRestore(); await fixture.dispose(); }
  });

  it.each([1, 2, 3, 4])("retains valid schema %i records, ineligible saves, and existing-first ties", async (version) => {
    const document = JSON.parse(waterContactRecord()) as { readonly header: Record<string, unknown> };
    const header = { ...document.header };
    delete header.personal_best_key;
    if (version < 4) delete header.physics_model_version;
    if (version < 3) delete header.score_definition_version;
    const legacy = JSON.stringify({ ...document, schema_version: version, header });
    const fixture = await createFixture("repair", legacy);
    const trace = tracedSelections();
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(fixture.factory, fixture.name), () => new Date(savedAt), trace.create);
    try {
      expect((await repository.saveFrom({ export_flight_record_json: () => legacy })).personalBest).toEqual({ kind: "ineligible" });
      expect(await repository.load(2)).toBe(legacy);
      expect(await repository.load(3)).toBe(legacy);
      expect((await repository.list()).filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([1]);
      expect((await repository.saveFrom({ export_flight_record_json: () => fixture.json })).personalBest).toMatchObject({ kind: "existing", id: 1 });
      expect(trace.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
      expect(fixture.connections.every(({ close }) => close.mock.calls.length === 1)).toBe(true);
    } finally { await fixture.dispose(); }
  });
});
