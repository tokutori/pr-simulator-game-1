import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { IDBFactory, IDBObjectStore as FakeObjectStore } from "fake-indexeddb";
import { afterEach, describe, expect, it, vi } from "vitest";
import { initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createArchivedPersonalBestSelection } from "../../web/src/game/archived-personal-best.js";
import {
  FlightRecordRepository, IndexedDbFlightRecordPersistence,
  type FlightRecordPersistencePort, type PersonalBestSelectionPort,
  type StoredFlightRecord, type StoredFlightRecordSummary
} from "../../web/src/game/flight-record-store.js";
import { currentRecordFixture } from "./current-session-fixture.js";

initSync({ module: new Uint8Array(readFileSync(fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url)))) });
afterEach(() => { vi.restoreAllMocks(); });

class MemoryFlightRecordPersistence implements FlightRecordPersistencePort {
  readonly records: StoredFlightRecord[] = [];
  private readonly personalBestIds = new Map<string, number>();
  add(json: string, savedAt: string, selection: PersonalBestSelectionPort): Promise<number> {
    for (const record of this.records) selection.consider_existing(record.id, record.json);
    const id = this.records.length + 1;
    this.records.push(Object.freeze({ id, savedAt, json }));
    if (selection.is_eligible()) {
      this.personalBestIds.set(selection.key_hex(), selection.candidate_is_best() ? id : selection.selected_existing_id());
    }
    return Promise.resolve(id);
  }
  get(id: number): Promise<StoredFlightRecord | null> {
    return Promise.resolve(this.records.find((record) => record.id === id) ?? null);
  }
  getAll(): Promise<readonly StoredFlightRecordSummary[]> {
    const best = new Set(this.personalBestIds.values());
    return Promise.resolve(this.records.map(({ id, savedAt }) => ({ id, savedAt, personalBest: best.has(id) })));
  }
}

class TestPersonalBestSelection implements PersonalBestSelectionPort {
  readonly consideredIds: number[] = [];
  freed = false;
  private existingId = 0;
  constructor(private readonly candidateWins = false, private readonly eligible = true) {}
  candidate_is_best(): boolean { return this.existingId === 0 || this.candidateWins; }
  consider_existing(id: number): void { this.consideredIds.push(id); this.existingId ||= id; }
  free(): void { this.freed = true; }
  is_eligible(): boolean { return this.eligible; }
  key_hex(): string { return "a".repeat(64); }
  selected_existing_id(): number { return this.existingId; }
}

function ineligibleSelection(): PersonalBestSelectionPort { return new TestPersonalBestSelection(false, false); }
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
async function databaseFixture(records: readonly unknown[] = [], metadata: readonly unknown[] = [],
  personalBests: readonly unknown[] = []) {
  const factory = new IDBFactory();
  const name = "current-flight-records";
  const request = factory.open(name, 4);
  request.onupgradeneeded = () => {
    request.result.createObjectStore("records", { keyPath: "id", autoIncrement: true });
    request.result.createObjectStore("recordMetadata", { keyPath: "id" });
    request.result.createObjectStore("personalBests", { keyPath: "key" });
  };
  const database = await requestResult(request);
  try {
    const transaction = database.transaction(["records", "recordMetadata", "personalBests"], "readwrite");
    const done = completed(transaction);
    for (const record of records) transaction.objectStore("records").put(record);
    for (const entry of metadata) transaction.objectStore("recordMetadata").put(entry);
    for (const entry of personalBests) transaction.objectStore("personalBests").put(entry);
    await done;
  } finally { database.close(); }
  return {
    factory, name, persistence: new IndexedDbFlightRecordPersistence(factory, name),
    async rows(store: string): Promise<unknown[]> {
      const connection = await requestResult(factory.open(name));
      try {
        const transaction = connection.transaction(store, "readonly");
        const done = completed(transaction);
        const rows = await requestResult(transaction.objectStore(store).getAll()) as unknown[];
        await done;
        return rows;
      } finally { connection.close(); }
    },
    async dispose(): Promise<void> { await requestResult(factory.deleteDatabase(name)); }
  };
}

describe("Current FlightRecord persistence", () => {
  it("preserves the Rust export and lists metadata newest first", async () => {
    const json = currentRecordFixture(false);
    const persistence = new MemoryFlightRecordPersistence();
    const repository = new FlightRecordRepository(persistence, () => new Date("2026-09-28T00:00:00.000Z"), createArchivedPersonalBestSelection);
    const first = await repository.saveFrom({ export_flight_record_json: () => json });
    const second = await repository.saveFrom({ export_flight_record_json: () => json });
    expect(first).toMatchObject({ id: 1, json, personalBest: { kind: "ineligible" } });
    expect(second.id).toBe(2);
    expect(await repository.load(1)).toBe(json);
    expect(await repository.list()).toEqual([
      { id: 2, savedAt: "2026-09-28T00:00:00.000Z", personalBest: false },
      { id: 1, savedAt: "2026-09-28T00:00:00.000Z", personalBest: false }
    ]);
  });

  it("uses the selection port and frees every candidate exactly once", async () => {
    const persistence = new MemoryFlightRecordPersistence();
    const selections: TestPersonalBestSelection[] = [];
    const repository = new FlightRecordRepository(persistence, undefined, () => {
      const selection = new TestPersonalBestSelection();
      selections.push(selection);
      return selection;
    });
    const json = currentRecordFixture();
    expect((await repository.saveFrom({ export_flight_record_json: () => json })).personalBest)
      .toEqual({ kind: "candidate", id: 1, key: "a".repeat(64) });
    expect((await repository.saveFrom({ export_flight_record_json: () => json })).personalBest)
      .toEqual({ kind: "existing", id: 1, key: "a".repeat(64) });
    expect(selections.map(({ consideredIds, freed }) => ({ consideredIds, freed }))).toEqual([
      { consideredIds: [], freed: true }, { consideredIds: [1], freed: true }
    ]);
  });

  it("scans current records once, then reuses the indexed Personal Best", async () => {
    const json = currentRecordFixture();
    const fixture = await databaseFixture([{ id: 1, savedAt: "first", json }], [{ id: 1, savedAt: "first" }]);
    try {
      const first = new TestPersonalBestSelection(true);
      expect(await fixture.persistence.add(json, "second", first)).toBe(2);
      expect(first.consideredIds).toEqual([1]);
      expect(await fixture.rows("personalBests")).toEqual([{ key: "a".repeat(64), recordId: 2 }]);
      const second = new TestPersonalBestSelection();
      expect(await fixture.persistence.add(json, "third", second)).toBe(3);
      expect(second.consideredIds).toEqual([2]);
      expect(await fixture.rows("records")).toHaveLength(3);
      expect(await fixture.rows("recordMetadata")).toHaveLength(3);
      expect(await fixture.rows("personalBests")).toEqual([{ key: "a".repeat(64), recordId: 2 }]);
    } finally { await fixture.dispose(); }
  });

  it("preserves the first current Rust Personal Best on equal scores", async () => {
    const fixture = await databaseFixture();
    try {
      const repository = new FlightRecordRepository(fixture.persistence, undefined, createArchivedPersonalBestSelection);
      const json = currentRecordFixture();
      expect((await repository.saveFrom({ export_flight_record_json: () => json })).personalBest).toMatchObject({ kind: "candidate", id: 1 });
      expect((await repository.saveFrom({ export_flight_record_json: () => json })).personalBest).toMatchObject({ kind: "existing", id: 1 });
      expect((await repository.list()).filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([1]);
    } finally { await fixture.dispose(); }
  });

  it.each([false, true])("retains unsupported stored bytes without blocking a current save (indexed=%s)", async (indexed) => {
    const json = JSON.stringify({ schema_version: 5, unused: "preserve stored bytes" });
    const current = currentRecordFixture();
    const selection = createArchivedPersonalBestSelection(current);
    let key: string;
    try { key = selection.key_hex(); } finally { selection.free(); }
    const fixture = await databaseFixture([{ id: 1, savedAt: "unsupported", json }],
      [{ id: 1, savedAt: "unsupported" }], indexed ? [{ key, recordId: 1 }] : []);
    try {
      const repository = new FlightRecordRepository(fixture.persistence, undefined, createArchivedPersonalBestSelection);
      expect((await repository.saveFrom({ export_flight_record_json: () => current })).personalBest).toMatchObject({ kind: "candidate", id: 2 });
      expect(await repository.load(1)).toBe(json);
      expect(await fixture.rows("records")).toHaveLength(2);
      await expect(repository.saveFrom({ export_flight_record_json: () => json })).rejects.toThrow(/Unsupported/);
    } finally { await fixture.dispose(); }
  });

  it("rolls back records, metadata and Personal Best when metadata insertion fails", async () => {
    const fixture = await databaseFixture();
    const json = currentRecordFixture();
    const add = Object.getOwnPropertyDescriptor(FakeObjectStore.prototype, "add")?.value as IDBObjectStore["add"];
    const spy = vi.spyOn(FakeObjectStore.prototype, "add").mockImplementation(function (this: IDBObjectStore, value: unknown, key?: IDBValidKey) {
      if (this.name === "recordMetadata") throw new Error("Injected metadata insertion failure");
      return add.call(this, value, key);
    });
    try {
      const repository = new FlightRecordRepository(fixture.persistence, undefined, createArchivedPersonalBestSelection);
      await expect(repository.saveFrom({ export_flight_record_json: () => json })).rejects.toThrow("Injected metadata insertion failure");
      spy.mockRestore();
      for (const store of ["records", "recordMetadata", "personalBests"]) expect(await fixture.rows(store)).toEqual([]);
      await expect(repository.saveFrom({ export_flight_record_json: () => json })).resolves.toMatchObject({ id: 1 });
    } finally { spy.mockRestore(); await fixture.dispose(); }
  });

  it.each([
    "not-json", JSON.stringify({ schema_version: 5 }),
    JSON.stringify({ schema_version: 6, header: [], samples: [{}], finalization: {} }),
    JSON.stringify({ schema_version: 6, header: {}, samples: [], finalization: {} }),
    JSON.stringify({ schema_version: 6, header: {}, samples: [{}], finalization: null })
  ])("rejects malformed or unsupported documents before storage", async (json) => {
    const persistence = new MemoryFlightRecordPersistence();
    const repository = new FlightRecordRepository(persistence, undefined, ineligibleSelection);
    await expect(repository.saveFrom({ export_flight_record_json: () => json })).rejects.toThrow();
    expect(persistence.records).toEqual([]);
  });

  it("rejects oversized documents and invalid identifiers", async () => {
    const repository = new FlightRecordRepository(new MemoryFlightRecordPersistence(), undefined, ineligibleSelection);
    const oversized = currentRecordFixture(false) + " ".repeat(16 * 1024 * 1024);
    await expect(repository.saveFrom({ export_flight_record_json: () => oversized })).rejects.toThrow(RangeError);
    for (const id of [0, -1, 1.5, Number.NaN]) await expect(repository.load(id)).rejects.toThrow(RangeError);
  });

  it("lists only valid metadata and marks the current indexed record", async () => {
    const fixture = await databaseFixture([], [{ id: 3, savedAt: "saved" }], [{ key: "a".repeat(64), recordId: 3 }]);
    try {
      expect(await fixture.persistence.getAll()).toEqual([{ id: 3, savedAt: "saved", personalBest: true }]);
      expect(await fixture.persistence.get(3)).toBeNull();
    } finally { await fixture.dispose(); }
  });

  it("reports malformed metadata and stored records", async () => {
    const metadata = await databaseFixture([], [{ id: "invalid", savedAt: "saved" }]);
    try { await expect(metadata.persistence.getAll()).rejects.toThrow("IndexedDB record metadata is malformed"); }
    finally { await metadata.dispose(); }
    const records = await databaseFixture([{ id: 3, savedAt: 42, json: currentRecordFixture(false) }]);
    try { await expect(records.persistence.get(3)).rejects.toThrow("IndexedDB flight record is malformed"); }
    finally { await records.dispose(); }
  });
});
