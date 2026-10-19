import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { IDBFactory, IDBObjectStore as FakeObjectStore } from "fake-indexeddb";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HybridGameSessionBridge, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { createArchivedPersonalBestSelection, personalBestComparisonVersion } from "../../web/src/game/archived-personal-best.js";
import {
  FlightRecordRepository, IndexedDbFlightRecordPersistence,
  type FlightRecordPersistencePort, type PersonalBestSelectionPort,
  type StoredFlightRecord, type StoredFlightRecordSummary
} from "../../web/src/game/flight-record-store.js";
import { currentRecordFixture } from "./current-session-fixture.js";
import { gameUiInputQuery } from "../../web/src/generated/game-ui-input-query.js";
import { encodeGameUiPlatformInput } from "../../web/src/app/game-ui-codec.js";
import { createSharedGameUiSession, sharedGameUiPlatform } from "./shared-game-ui-fixture.js";

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
  getRecent(limit: number): Promise<readonly StoredFlightRecordSummary[]> {
    const best = new Set(this.personalBestIds.values());
    return Promise.resolve([...this.records].sort((left, right) => right.id - left.id).slice(0, limit)
      .map(({ id, savedAt }) => ({ id, savedAt, personalBest: best.has(id) })));
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
    factory, name, persistence: new IndexedDbFlightRecordPersistence(factory, personalBestComparisonVersion(), name),
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
  it("keeps the Title query bounded with 2000 saved records while retaining an older Personal Best", async () => {
    const json = currentRecordFixture();
    const selection = createArchivedPersonalBestSelection(json);
    let key: string;
    try { key = selection.key_hex(); } finally { selection.free(); }
    const records = Array.from({ length: 2000 }, (_, index) => ({
      id: index + 1, savedAt: "2026-10-19T00:00:00.000Z", json
    }));
    const fixture = await databaseFixture(records, records.map(({ id, savedAt }) => ({ id, savedAt })), [{ key, recordId: 1, comparisonVersion: personalBestComparisonVersion() }]);
    try {
      const repository = new FlightRecordRepository(fixture.persistence, undefined, createArchivedPersonalBestSelection);
      const getAll = vi.spyOn(FakeObjectStore.prototype, "getAll");
      const recent = await repository.recent(gameUiInputQuery.storedRecordLimit);
      expect(recent.map(({ id }) => id)).toEqual([2000, 1999, 1998]);
      expect(getAll.mock.contexts.filter((store) => store instanceof FakeObjectStore).map((store) => store.name)).not.toContain("recordMetadata");
      const platform = { ...sharedGameUiPlatform(), stored_records: recent.map(({ id, savedAt, personalBest }) => ({
        id, saved_at: savedAt, personal_best: personalBest
      })) };
      const input = encodeGameUiPlatformInput(platform);
      expect(new TextEncoder().encode(input).byteLength).toBeLessThan(gameUiInputQuery.maximumBytes);
      const unbounded = JSON.stringify({ ...platform, stored_records: records.map(({ id, savedAt }) => ({
        id, saved_at: savedAt, personal_best: id === 1
      })) });
      expect(new TextEncoder().encode(unbounded).byteLength).toBeGreaterThan(gameUiInputQuery.maximumBytes);
      const boundary = new HybridGameSessionBridge(0, 21, 22);
      try {
        expect(() => boundary.game_ui_view_json(unbounded)).toThrow("shared UI input exceeds the boundary limit");
        expect(() => boundary.game_ui_view_json(input)).not.toThrow();
      } finally { boundary.free(); }
      const title = createSharedGameUiSession(0);
      try {
        const view = title.session.readSharedUi(platform);
        const recordIds = view.panels.flatMap((panel) => panel.controls)
          .filter((control) => control.id.startsWith("game-title-open-record-")).map(({ id }) => id);
        expect(recordIds).toEqual([2000, 1999, 1998].map((id) => `game-title-open-record-${String(id)}`));
        expect(title.session.readSharedUi(platform).scene).toBe("Title");
        expect(title.session.updateSharedUi({ kind: "platform", command: { kind: "open_stored_record", id: 2000 } }, platform).effects)
          .toEqual([{ kind: "platform", command: { kind: "open_stored_record", id: 2000 } }]);
      } finally { title.session.dispose(); }
      const flight = createSharedGameUiSession(5);
      try {
        const before = flight.session.readSnapshot();
        expect(flight.session.readSharedUi(platform).scene).toBe("Flight");
        expect(flight.session.readSharedUi(platform).scene).toBe("Flight");
        expect(flight.session.readSnapshot()).toEqual(before);
        expect(flight.session.updateSharedUi({ kind: "session", command: { kind: "pause" } }, platform).effects)
          .toEqual([{ kind: "session", command: { kind: "pause" } }]);
      } finally { flight.session.dispose(); }
      expect((await repository.saveFrom({ export_flight_record_json: () => json })).personalBest)
        .toMatchObject({ kind: "existing", id: 1 });
      expect(await fixture.rows("personalBests")).toEqual([{ key, recordId: 1, comparisonVersion: personalBestComparisonVersion() }]);
      expect(await repository.load(1)).toBe(json);
      expect(await fixture.rows("recordMetadata")).toHaveLength(2001);
    } finally { await fixture.dispose(); }
  });

  it("bounds Unicode display diagnostics without changing the original platform observations", () => {
    const text = "🛩\u0000日本語".repeat(100_000);
    const platform = sharedGameUiPlatform({ notification: text, flight_controller_failure: text, head_hud_failure: text,
      water_quality_operation: { kind: "failed", requested: "high", cause: text },
      screen_render_scale: { ...sharedGameUiPlatform().screen_render_scale, status: { kind: "unsupported", cause: text } },
      stored_records: [{ id: 1, saved_at: text, personal_best: false }] });
    const input = encodeGameUiPlatformInput(platform);
    expect(new TextEncoder().encode(input).byteLength).toBeLessThan(gameUiInputQuery.maximumBytes);
    expect(platform.notification).toBe(text);
    expect(platform.stored_records[0]?.saved_at).toBe(text);
    const owner = createSharedGameUiSession(0);
    try {
      expect(owner.session.readSharedUi(platform).scene).toBe("Title");
    } finally { owner.session.dispose(); }
    expect(() => encodeGameUiPlatformInput({ ...sharedGameUiPlatform(),
      stored_records: Array.from({ length: 4 }, (_, index) => ({ id: index + 1, saved_at: "saved", personal_best: false }))
    })).toThrow();
  });

  it("rejects invalid preview limits and retains metadata/index errors on the bounded query", async () => {
    const fixture = await databaseFixture([], [{ id: 1, savedAt: 7 }]);
    try {
      const repository = new FlightRecordRepository(fixture.persistence, undefined, createArchivedPersonalBestSelection);
      for (const limit of [0, -1, 0.5, Number.POSITIVE_INFINITY]) await expect(repository.recent(limit)).rejects.toThrow("positive integer");
      await expect(repository.recent(gameUiInputQuery.storedRecordLimit)).rejects.toThrow("metadata is malformed");
    } finally { await fixture.dispose(); }
    const badIndex = await databaseFixture([], [], [{ key: "invalid", recordId: 1 }]);
    try {
      await expect(badIndex.persistence.getRecent(3)).rejects.toThrow("Personal Best index is malformed");
    } finally { await badIndex.dispose(); }
  });

  it("preserves the Rust export and lists metadata newest first", async () => {
    const json = currentRecordFixture(false);
    const persistence = new MemoryFlightRecordPersistence();
    const repository = new FlightRecordRepository(persistence, () => new Date("2026-09-28T00:00:00.000Z"), createArchivedPersonalBestSelection);
    const first = await repository.saveFrom({ export_flight_record_json: () => json });
    const second = await repository.saveFrom({ export_flight_record_json: () => json });
    expect(first).toMatchObject({ id: 1, json, personalBest: { kind: "ineligible" } });
    expect(second.id).toBe(2);
    expect(await repository.load(1)).toBe(json);
    expect(await repository.recent(gameUiInputQuery.storedRecordLimit)).toEqual([
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
      expect(await fixture.rows("personalBests")).toEqual([{ key: "a".repeat(64), recordId: 2, comparisonVersion: personalBestComparisonVersion() }]);
      const second = new TestPersonalBestSelection();
      expect(await fixture.persistence.add(json, "third", second)).toBe(3);
      expect(second.consideredIds).toEqual([2]);
      expect(await fixture.rows("records")).toHaveLength(3);
      expect(await fixture.rows("recordMetadata")).toHaveLength(3);
      expect(await fixture.rows("personalBests")).toEqual([{ key: "a".repeat(64), recordId: 2, comparisonVersion: personalBestComparisonVersion() }]);
    } finally { await fixture.dispose(); }
  });

  it("preserves the first current Rust Personal Best on equal scores", async () => {
    const fixture = await databaseFixture();
    try {
      const repository = new FlightRecordRepository(fixture.persistence, undefined, createArchivedPersonalBestSelection);
      const json = currentRecordFixture();
      expect((await repository.saveFrom({ export_flight_record_json: () => json })).personalBest).toMatchObject({ kind: "candidate", id: 1 });
      expect((await repository.saveFrom({ export_flight_record_json: () => json })).personalBest).toMatchObject({ kind: "existing", id: 1 });
      expect((await repository.recent(gameUiInputQuery.storedRecordLimit)).filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([1]);
    } finally { await fixture.dispose(); }
  });

  it.each([undefined, 999])("keeps old or unknown index revisions unranked and scans current candidates (revision=%s)", async (revision) => {
    const json = currentRecordFixture();
    const selection = createArchivedPersonalBestSelection(json);
    let key: string;
    try { key = selection.key_hex(); } finally { selection.free(); }
    const old = JSON.parse(json) as Record<string, unknown>;
    delete old.identity_metadata;
    const unknown = { ...old, identity_metadata: { comparison_version: 999, future: "原文を保持する" } };
    const records = [
      { id: 1, savedAt: "old", json: JSON.stringify(old) },
      { id: 2, savedAt: "unknown", json: JSON.stringify(unknown) },
      { id: 3, savedAt: "current", json }
    ];
    const index = revision === undefined ? { key, recordId: 1 } : { key, recordId: 1, comparisonVersion: revision };
    const fixture = await databaseFixture(records, records.map(({ id, savedAt }) => ({ id, savedAt })), [index]);
    try {
      const repository = new FlightRecordRepository(fixture.persistence, undefined, createArchivedPersonalBestSelection);
      expect((await repository.recent(3)).every(({ personalBest }) => !personalBest)).toBe(true);
      expect(await fixture.rows("personalBests")).toEqual([index]);
      expect((await repository.saveFrom({ export_flight_record_json: () => json })).personalBest).toMatchObject({ kind: "existing", id: 3 });
      expect((await repository.recent(3)).filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([3]);
      expect(await repository.load(1)).toBe(records[0]?.json);
      expect(await repository.load(2)).toBe(records[1]?.json);
      expect(await fixture.rows("personalBests")).toEqual([{ key, recordId: 3, comparisonVersion: personalBestComparisonVersion() }]);
    } finally { await fixture.dispose(); }
  });

  it.each([false, true])("retains unsupported stored bytes without blocking a current save (indexed=%s)", async (indexed) => {
    const json = JSON.stringify({ schema_version: 5, unused: "preserve stored bytes" });
    const current = currentRecordFixture();
    const selection = createArchivedPersonalBestSelection(current);
    let key: string;
    try { key = selection.key_hex(); } finally { selection.free(); }
    const fixture = await databaseFixture([{ id: 1, savedAt: "unsupported", json }],
      [{ id: 1, savedAt: "unsupported" }], indexed ? [{ key, recordId: 1, comparisonVersion: personalBestComparisonVersion() }] : []);
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
    const fixture = await databaseFixture([], [{ id: 3, savedAt: "saved" }], [{ key: "a".repeat(64), recordId: 3, comparisonVersion: personalBestComparisonVersion() }]);
    try {
      expect(await fixture.persistence.getRecent(3)).toEqual([{ id: 3, savedAt: "saved", personalBest: true }]);
      expect(await fixture.persistence.get(3)).toBeNull();
    } finally { await fixture.dispose(); }
  });

  it("reports malformed metadata and stored records", async () => {
    const metadata = await databaseFixture([], [{ id: "invalid", savedAt: "saved" }]);
    try { await expect(metadata.persistence.getRecent(3)).rejects.toThrow("IndexedDB record metadata is malformed"); }
    finally { await metadata.dispose(); }
    const records = await databaseFixture([{ id: 3, savedAt: 42, json: currentRecordFixture(false) }]);
    try { await expect(records.persistence.get(3)).rejects.toThrow("IndexedDB flight record is malformed"); }
    finally { await records.dispose(); }
  });
});
