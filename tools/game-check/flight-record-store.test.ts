import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, expect, it, vi } from "vitest";
import { IDBCursor as FakeCursor, IDBFactory, IDBObjectStore as FakeObjectStore } from "fake-indexeddb";
import { GameSessionBridge, HybridGameSessionBridge, PersonalBestSelectionBridge, compare_personal_best_json, initSync } from "../../web/pkg/birdman_game_wasm.js";
import { parseFlightSnapshot } from "../../web/src/game/flight-snapshot.js";
import { createArchivedPersonalBestSelection } from "../../web/src/game/archived-personal-best.js";
import { encodeTailLogicalInput } from "../../web/src/game/tail-session-codec.js";
import {
  FlightRecordRepository,
  IndexedDbFlightRecordPersistence,
  type FlightRecordPersistencePort,
  type PersonalBestSelectionPort,
  type StoredFlightRecord,
  type StoredFlightRecordSummary
} from "../../web/src/game/flight-record-store.js";

const currentPersonalBestIndexRevision = "first-winner-layouts-v1-physics-v3";

class MemoryFlightRecordPersistence implements FlightRecordPersistencePort {
  private readonly records: StoredFlightRecord[] = [];
  private readonly personalBestIds = new Map<string, number>();

  add(json: string, savedAt: string, selection?: PersonalBestSelectionPort): Promise<number> {
    for (const record of this.records) selection?.consider_existing(record.id, record.json);
    const id = this.records.length + 1;
    this.records.push(Object.freeze({ id, savedAt, json }));
    if (selection?.is_eligible()) {
      const selectedId = selection.candidate_is_best() ? id : selection.selected_existing_id();
      this.personalBestIds.set(selection.key_hex(), selectedId);
    }
    return Promise.resolve(id);
  }

  get(id: number): Promise<StoredFlightRecord | null> {
    return Promise.resolve(this.records.find((record) => record.id === id) ?? null);
  }

  getAll(): Promise<readonly StoredFlightRecordSummary[]> {
    const personalBestIds = new Set(this.personalBestIds.values());
    return Promise.resolve(this.records.map(({ id, savedAt }) => Object.freeze({
      id,
      savedAt,
      personalBest: personalBestIds.has(id)
    })));
  }
}

const finalizedRecord = JSON.stringify({
  schema_version: 1,
  header: { scenario_id: 1 },
  samples: [{ tick_index: 0 }],
  finalization: { reason: "manual_abort" }
});

const finalizedCustomHudRecord = JSON.stringify({
  schema_version: 4,
  header: {
    scenario_id: 1,
    score_definition_version: 1,
    physics_model_version: 1,
    difficulty: { information: "custom", hud_profile: {} }
  },
  samples: [{ tick_index: 0 }],
  finalization: { reason: "manual_abort" }
});

describe("FlightRecordRepository", () => {
  it("persists the Rust-exported finalized document and returns records newest first", async () => {
    const persistence = new MemoryFlightRecordPersistence();
    const repository = new FlightRecordRepository(persistence, () => new Date("2026-09-28T00:00:00.000Z"), createIneligibleSelection);
    const first = await repository.saveFrom({ export_flight_record_json: () => finalizedRecord });
    const second = await repository.saveFrom({ export_flight_record_json: () => finalizedRecord });

    expect(first).toMatchObject({ id: 1, json: finalizedRecord });
    expect(second.id).toBe(2);
    expect(await repository.load(1)).toBe(finalizedRecord);
    const entries = await repository.list();
    expect(entries.map((record) => record.id)).toEqual([2, 1]);
    expect(entries[0]).toEqual({ id: 2, savedAt: "2026-09-28T00:00:00.000Z", personalBest: false });
    expect(entries[0]).not.toHaveProperty("json");
  });

  it("persists current schema records without coupling the browser adapter to a schema version", async () => {
    const persistence = new MemoryFlightRecordPersistence();
    const repository = new FlightRecordRepository(persistence, undefined, createIneligibleSelection);

    const saved = await repository.saveFrom({ export_flight_record_json: () => finalizedCustomHudRecord });

    expect(saved.json).toBe(finalizedCustomHudRecord);
    expect(await repository.load(saved.id)).toBe(finalizedCustomHudRecord);
  });

  it("uses the Rust selection port for persisted Personal Best outcomes", async () => {
    const persistence = new MemoryFlightRecordPersistence();
    const selections: TestPersonalBestSelection[] = [];
    const repository = new FlightRecordRepository(persistence, undefined, () => {
      const selection = new TestPersonalBestSelection();
      selections.push(selection);
      return selection;
    });

    const first = await repository.saveFrom({ export_flight_record_json: () => finalizedRecord });
    const second = await repository.saveFrom({ export_flight_record_json: () => finalizedRecord });

    expect(first.personalBest).toEqual({ kind: "candidate", id: 1, key: "a".repeat(64) });
    expect(second.personalBest).toEqual({ kind: "existing", id: 1, key: "a".repeat(64) });
    expect(selections.map(({ consideredIds, freed }) => ({ consideredIds, freed }))).toEqual([
      { consideredIds: [], freed: true },
      { consideredIds: [1], freed: true }
    ]);
  });

  it("updates the IndexedDB Personal Best index atomically and reuses it", async () => {
    const database = memoryIndexedDb([
      { id: 1, savedAt: "2026-09-29T00:00:00.000Z", json: finalizedRecord }
    ]);
    const persistence = new IndexedDbFlightRecordPersistence(database.factory);
    const firstSelection = new TestPersonalBestSelection(true);
    expect(await persistence.add(finalizedRecord, "2026-09-30T00:00:00.000Z", firstSelection, () => new TestPersonalBestSelection())).toBe(2);
    expect(firstSelection.consideredIds).toEqual([1]);
    expect(database.personalBests.get("a".repeat(64))).toEqual({ key: "a".repeat(64), recordId: 2 });

    const secondSelection = new TestPersonalBestSelection(false);
    expect(await persistence.add(finalizedRecord, "2026-10-01T00:00:00.000Z", secondSelection, () => new TestPersonalBestSelection())).toBe(3);
    expect(secondSelection.consideredIds).toEqual([2]);
    expect(database.cursorCount()).toBe(1);
    expect(database.records()).toHaveLength(3);
    expect(database.metadata()).toHaveLength(3);
    expect(database.personalBests.get("a".repeat(64))?.recordId).toBe(2);
  });

  it("rebuilds the canonical Personal Best index once for pre-index records", async () => {
    const database = memoryIndexedDb([
      { id: 1, savedAt: "2026-09-28T00:00:00.000Z", json: finalizedRecord },
      { id: 2, savedAt: "2026-09-29T00:00:00.000Z", json: finalizedRecord }
    ]);
    const selections: TestPersonalBestSelection[] = [];
    const repository = new FlightRecordRepository(
      new IndexedDbFlightRecordPersistence(database.factory),
      undefined,
      () => {
        const selection = new TestPersonalBestSelection();
        selections.push(selection);
        return selection;
      }
    );

    await expect(repository.list()).resolves.toEqual([
      { id: 2, savedAt: "2026-09-29T00:00:00.000Z", personalBest: false },
      { id: 1, savedAt: "2026-09-28T00:00:00.000Z", personalBest: true }
    ]);
    expect(database.personalBests.get("a".repeat(64))).toEqual({ key: "a".repeat(64), recordId: 1 });
    expect(database.personalBestIndexInitialized()).toBe(true);
    expect(database.cursorCount()).toBe(1);
    expect(selections.every(({ freed }) => freed)).toBe(true);

    await repository.list();
    expect(database.cursorCount()).toBe(1);
  });

  it.each([1, 2, 3])("upgrades a version-%i database and preserves its records", async (version) => {
    const factory = new IDBFactory();
    const databaseName = `flight-record-upgrade-${String(Date.now())}-${String(Math.random())}`;
    const personalBestEligible = version === 3;
    await seedVersionedDatabase(factory, databaseName, version, [{
      id: 1,
      savedAt: "2026-09-29T00:00:00.000Z",
      json: personalBestEligible ? JSON.stringify({ schema_version: 5 }) : finalizedRecord
    }]);
    const repository = new FlightRecordRepository(
      new IndexedDbFlightRecordPersistence(factory, databaseName),
      undefined,
      () => new TestPersonalBestSelection(false, personalBestEligible)
    );

    await expect(repository.list()).resolves.toEqual([
      { id: 1, savedAt: "2026-09-29T00:00:00.000Z", personalBest: personalBestEligible }
    ]);
    await expect(readDatabaseVersion(factory, databaseName)).resolves.toBe(4);
    const database = await openTestDatabase(factory, databaseName);
    try {
      expect([...database.objectStoreNames]).toEqual([
        "personalBestIndexState",
        "personalBests",
        "recordMetadata",
        "records"
      ]);
      const transaction = database.transaction(
        ["recordMetadata", "personalBests", "personalBestIndexState"],
        "readonly"
      );
      const metadataRequest = requestResult(transaction.objectStore("recordMetadata").get(1));
      const personalBestRequest = requestResult(transaction.objectStore("personalBests").getAll());
      const indexStateRequest = requestResult(transaction.objectStore("personalBestIndexState").get(currentPersonalBestIndexRevision));
      await expect(metadataRequest).resolves.toEqual({
        id: 1,
        savedAt: "2026-09-29T00:00:00.000Z"
      });
      await expect(personalBestRequest).resolves.toEqual(personalBestEligible
        ? [{ key: "a".repeat(64), recordId: 1 }]
        : []);
      await expect(indexStateRequest).resolves.toEqual({ key: currentPersonalBestIndexRevision });
    } finally {
      database.close();
      await deleteDatabase(factory, databaseName);
    }
  });

  it("rolls back the record insert when metadata insertion fails", async () => {
    const factory = new IDBFactory();
    const databaseName = `flight-record-rollback-${String(Date.now())}-${String(Math.random())}`;
    await seedVersionedDatabase(factory, databaseName, 3, [], [{ id: 1, savedAt: "stale" }]);
    const repository = new FlightRecordRepository(
      new IndexedDbFlightRecordPersistence(factory, databaseName),
      undefined,
      () => new TestPersonalBestSelection()
    );

    try {
      await expect(repository.saveFrom({ export_flight_record_json: () => finalizedRecord })).rejects.toThrow();
      await expect(repository.load(1)).resolves.toBeNull();
      await expect(repository.list()).resolves.toEqual([
        { id: 1, savedAt: "stale", personalBest: false }
      ]);
    } finally {
      await deleteDatabase(factory, databaseName);
    }
  });

  it.each([
    "not-json",
    JSON.stringify({ schema_version: 2, header: [], samples: [{}], finalization: {} }),
    JSON.stringify({ schema_version: 1, header: [], samples: [{}], finalization: {} }),
    JSON.stringify({ schema_version: 1, header: {}, samples: [], finalization: {} }),
    JSON.stringify({ schema_version: 1, header: {}, samples: [{}], finalization: null })
  ])("rejects invalid or incomplete documents before storage", async (json) => {
    const persistence = new MemoryFlightRecordPersistence();
    const repository = new FlightRecordRepository(persistence, undefined, createIneligibleSelection);
    await expect(repository.saveFrom({ export_flight_record_json: () => json })).rejects.toThrow();
    expect(await repository.list()).toEqual([]);
  });

  it("rejects documents above the Rust format size limit", async () => {
    const persistence = new MemoryFlightRecordPersistence();
    const repository = new FlightRecordRepository(persistence, undefined, createIneligibleSelection);
    const oversized = `${finalizedRecord}${" ".repeat(16 * 1024 * 1024)}`;
    await expect(repository.saveFrom({ export_flight_record_json: () => oversized })).rejects.toThrow(RangeError);
    expect(await repository.list()).toEqual([]);
  });

  it("rejects invalid record identifiers", async () => {
    const repository = new FlightRecordRepository(new MemoryFlightRecordPersistence(), undefined, createIneligibleSelection);
    await expect(repository.load(0)).rejects.toThrow(RangeError);
  });

  it("returns IndexedDB metadata when valid and reports malformed rows", async () => {
    const valid = new IndexedDbFlightRecordPersistence(indexedDbFactoryWithReadResult("recordMetadata", [
      { id: 3, savedAt: "2026-09-30T00:00:00.000Z" }
    ]));
    await expect(valid.getAll(createIneligibleSelection)).resolves.toEqual([
      { id: 3, savedAt: "2026-09-30T00:00:00.000Z", personalBest: false }
    ]);

    const personalBest = new IndexedDbFlightRecordPersistence(indexedDbFactoryWithReadResult(
      "recordMetadata",
      [{ id: 3, savedAt: "2026-09-30T00:00:00.000Z" }],
      [{ key: "a".repeat(64), recordId: 3 }]
    ));
    await expect(personalBest.getAll(createIneligibleSelection)).resolves.toEqual([
      { id: 3, savedAt: "2026-09-30T00:00:00.000Z", personalBest: true }
    ]);

    const malformed = new IndexedDbFlightRecordPersistence(indexedDbFactoryWithReadResult("recordMetadata", [
      { id: 3, savedAt: "2026-09-30T00:00:00.000Z" },
      { id: "invalid", savedAt: "2026-09-30T00:00:00.000Z" }
    ]));
    await expect(malformed.getAll(createIneligibleSelection)).rejects.toThrow("IndexedDB record metadata is malformed");

    const missing = new IndexedDbFlightRecordPersistence(indexedDbFactoryWithReadResult("records", undefined));
    await expect(missing.get(3)).resolves.toBeNull();

    const malformedRecord = new IndexedDbFlightRecordPersistence(indexedDbFactoryWithReadResult("records", {
      id: 0,
      savedAt: "2026-09-30T00:00:00.000Z",
      json: finalizedRecord
    }));
    await expect(malformedRecord.get(3)).rejects.toThrow("IndexedDB flight record is malformed");
  });
});

describe("Personal Best index repair", () => {
  it.each(["list", "save"] as const)("repairs a completed legacy index across saved schemas 1–6 through first %s", async (firstOperation) => {
    const legacy = waterContactRecord();
    const tail = tailWaterContactRecord();
    const legacySelection = createArchivedPersonalBestSelection(legacy);
    const tailSelection = createArchivedPersonalBestSelection(tail);
    let legacyKey: string;
    let tailKey: string;
    try {
      expect(legacySelection.is_eligible()).toBe(true);
      expect(tailSelection.is_eligible()).toBe(true);
      legacyKey = legacySelection.key_hex();
      tailKey = tailSelection.key_hex();
      expect(tailKey).not.toBe(legacyKey);
    } finally {
      legacySelection.free();
      tailSelection.free();
    }
    const records = [
      { id: 1, savedAt: "legacy-first", json: legacy },
      { id: 2, savedAt: "tail-first", json: tail },
      { id: 3, savedAt: "legacy-tie", json: legacy },
      { id: 4, savedAt: "tail-tie", json: tail },
      ...([1, 2, 3, 4] as const).map((version) => ({ id: version + 4,
        savedAt: `legacy-schema-${String(version)}`, json: legacyArchiveRecord(legacy, version) }))
    ];
    const factory = new IDBFactory();
    const databaseName = `pb-mixed-layout-repair-${firstOperation}`;
    await seedPersonalBestDatabase(factory, databaseName, 4, records,
      [{ key: legacyKey, recordId: 3 }, { key: tailKey, recordId: 4 }], false);
    const database = await openTestDatabase(factory, databaseName);
    try {
      const transaction = database.transaction("personalBestIndexState", "readwrite");
      transaction.objectStore("personalBestIndexState").put({ key: "first-winner-physics-v3" });
      await new Promise<void>((resolve, reject) => {
        transaction.oncomplete = () => { resolve(); };
        transaction.onabort = () => { reject(transaction.error ?? new Error("Legacy index marker seed aborted")); };
      });
    } finally {
      database.close();
    }
    const selections: { freeCount: number }[] = [];
    const createSelection = (json: string): PersonalBestSelectionPort => {
      const selection = createArchivedPersonalBestSelection(json);
      const trace = { freeCount: 0 };
      selections.push(trace);
      return Object.freeze({ ...selection, free: () => { trace.freeCount += 1; selection.free(); } });
    };
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(factory, databaseName), undefined, createSelection);
    try {
      if (firstOperation === "save") {
        expect((await repository.saveFrom({ export_flight_record_json: () => tail })).personalBest)
          .toEqual({ kind: "existing", id: 2, key: tailKey });
      }
      const listed = await repository.list();
      expect(listed.filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([2, 1]);
      expect(listed).toHaveLength(records.length + (firstOperation === "save" ? 1 : 0));
      for (const record of records) expect(await repository.load(record.id)).toBe(record.json);
      const repaired = await readPersonalBestDatabase(factory, databaseName);
      const repairedRecords = repaired.records;
      if (repairedRecords === undefined) throw new Error("Repaired records store is missing");
      expect(repairedRecords.slice(0, records.length)).toEqual(records);
      expect(repaired.personalBests).toHaveLength(2);
      expect(repaired.personalBests).toEqual(expect.arrayContaining([{ key: legacyKey, recordId: 1 }, { key: tailKey, recordId: 2 }]));
      expect(repaired.personalBestIndexState).toEqual(expect.arrayContaining([
        { key: "first-winner-physics-v3" }, { key: currentPersonalBestIndexRevision }
      ]));
      const creations = selections.length;
      await repository.list();
      expect(selections).toHaveLength(creations);
      const saved = await repository.saveFrom({ export_flight_record_json: () => tail });
      expect(saved.personalBest).toEqual({ kind: "existing", id: 2, key: tailKey });
      expect(await repository.load(saved.id)).toBe(tail);
      expect(selections.every(({ freeCount }) => freeCount === 1)).toBe(true);
    } finally {
      await deleteDatabase(factory, databaseName);
    }
  });

  it.each([
    [3, "list", 2, false], [3, "save", 2, false], [3, "list", 3, false], [3, "save", 3, false],
    [4, "list", 2, true], [4, "save", 2, true], [4, "list", 3, true], [4, "save", 3, true],
    [4, "list", 2, false], [4, "save", 2, false]
  ] as const)("preserves the first Rust winner for database %i, first %s, %i ties, legacy marker %s", async (version, firstOperation, count, legacyMarker) => {
    const json = waterContactRecord();
    const factory = new IDBFactory();
    const databaseName = `pb-repair-${String(version)}-${firstOperation}-${String(count)}`;
    const records = Array.from({ length: count }, (_, index) => ({
      id: index + 1, savedAt: `2026-09-${String(20 + index)}T00:00:00.000Z`, json
    }));
    const selection = new PersonalBestSelectionBridge(json);
    const key = selection.key_hex();
    try {
      selection.consider_existing(1, json);
      expect(selection.candidate_is_best()).toBe(false);
      expect(selection.selected_existing_id()).toBe(1);
    } finally {
      selection.free();
    }
    await seedPersonalBestDatabase(factory, databaseName, version, records, [{ key, recordId: version === 3 ? 1 : 2 }], legacyMarker);
    const repository = new FlightRecordRepository(
      new IndexedDbFlightRecordPersistence(factory, databaseName), undefined,
      (recordJson) => new PersonalBestSelectionBridge(recordJson)
    );
    try {
      if (firstOperation === "save") {
        const saved = await repository.saveFrom({ export_flight_record_json: () => json });
        expect(saved.personalBest).toEqual({ kind: "existing", id: 1, key });
      }
      expect((await repository.list()).filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([1]);
      for (const record of records) expect(await repository.load(record.id)).toBe(record.json);
      const database = await openTestDatabase(factory, databaseName);
      try {
        const transaction = database.transaction(["records", "personalBests", "personalBestIndexState"], "readonly");
        const stored = requestResult(transaction.objectStore("records").getAll());
        const winner = requestResult(transaction.objectStore("personalBests").get(key));
        const marker = requestResult(transaction.objectStore("personalBestIndexState").get(currentPersonalBestIndexRevision));
        expect((await stored).slice(0, records.length)).toEqual(records);
        await expect(winner).resolves.toEqual({ key, recordId: 1 });
        await expect(marker).resolves.toEqual({ key: currentPersonalBestIndexRevision });
      } finally {
        database.close();
      }
      const saved = await repository.saveFrom({ export_flight_record_json: () => json });
      expect(saved.personalBest).toEqual({ kind: "existing", id: 1, key });
      expect((await repository.list()).filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([1]);
    } finally {
      await deleteDatabase(factory, databaseName);
    }
  });

  it.each(["list", "save"] as const)("rebuilds past physics winners through first %s despite completed old markers", async (firstOperation) => {
    const currentJson = waterContactRecord();
    const current = JSON.parse(currentJson) as {
      readonly header: { readonly physics_model_version: number; readonly personal_best_key: readonly number[] };
    };
    const currentVersion = current.header.physics_model_version;
    expect(currentVersion).toBeGreaterThan(1);
    const oldRecords = Array.from({ length: currentVersion - 1 }, (_, index) => {
      const version = index + 1;
      const json = JSON.stringify({ ...current, header: {
        ...current.header, physics_model_version: version, personal_best_key: Array.from({ length: 32 }, () => version)
      } });
      const oldSelection = new PersonalBestSelectionBridge(json);
      try { expect(oldSelection.is_eligible()).toBe(false); } finally { oldSelection.free(); }
      return { id: version, savedAt: `physics-${String(version)}`, json };
    });
    const currentId = currentVersion;
    const records = [...oldRecords,
      { id: currentId, savedAt: "current-first", json: currentJson },
      { id: currentId + 1, savedAt: "current-tie", json: currentJson }
    ];
    const currentSelection = new PersonalBestSelectionBridge(currentJson);
    let currentKey: string;
    try { currentKey = currentSelection.key_hex(); } finally { currentSelection.free(); }
    const oldIndex = oldRecords.map(({ id }) => ({ key: id.toString(16).padStart(2, "0").repeat(32), recordId: id }));
    const factory = new IDBFactory();
    const databaseName = `pb-physics-upgrade-${firstOperation}`;
    await seedPersonalBestDatabase(factory, databaseName, 4, records, [...oldIndex, { key: currentKey, recordId: currentId + 1 }], false);
    const database = await openTestDatabase(factory, databaseName);
    try {
      const transaction = database.transaction(["personalBestIndexState"], "readwrite");
      transaction.objectStore("personalBestIndexState").put({ key: "first-winner-v1" });
      for (const { id } of oldRecords) {
        transaction.objectStore("personalBestIndexState").put({ key: `first-winner-physics-v${String(id)}` });
      }
      await new Promise<void>((resolve, reject) => {
        transaction.oncomplete = () => { resolve(); };
        transaction.onabort = () => { reject(transaction.error ?? new Error("Physics marker seed aborted")); };
      });
    } finally { database.close(); }
    const traced = tracedRustSelections();
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(factory, databaseName), undefined, traced.create);
    try {
      if (firstOperation === "save") {
        expect((await repository.saveFrom({ export_flight_record_json: interruptedRecord })).personalBest).toEqual({ kind: "ineligible" });
      }
      expect((await repository.list()).filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([currentId]);
      const repaired = await readPersonalBestDatabase(factory, databaseName);
      expect(repaired.personalBests).toEqual([{ key: currentKey, recordId: currentId }]);
      expect(repaired.personalBestIndexState).toContainEqual({ key: currentPersonalBestIndexRevision });
      for (const record of records) expect(await repository.load(record.id)).toBe(record.json);
      const creations = traced.entries.length;
      await repository.list();
      expect(traced.entries).toHaveLength(creations);
      expect(traced.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
    } finally { await deleteDatabase(factory, databaseName); }
  });

  it("uses Rust scores and keys across rebuild, later saves, and ineligible records", async () => {
    const neutral = waterContactRecord();
    const pitched = waterContactRecord(0.1);
    const ranking = new PersonalBestSelectionBridge(neutral);
    let higher: string;
    let lower: string;
    try {
      ranking.consider_existing(1, pitched);
      higher = ranking.candidate_is_best() ? neutral : pitched;
      lower = ranking.candidate_is_best() ? pitched : neutral;
    } finally {
      ranking.free();
    }
    const opposite = new PersonalBestSelectionBridge(lower);
    try {
      expect(compare_personal_best_json(lower, higher)).not.toBe(2);
      opposite.consider_existing(1, higher);
      expect(opposite.selected_existing_id()).toBe(1);
    } finally {
      opposite.free();
    }
    const document = JSON.parse(neutral) as { readonly header: Record<string, unknown> };
    const otherKey = JSON.stringify({ ...document, header: { ...document.header, personal_best_key: Array.from({ length: 32 }, () => 0) } });
    const ineligible = interruptedRecord();
    const legacy = JSON.stringify({ ...document, schema_version: 4, header: { ...document.header, personal_best_key: null } });
    const jsonRecords = [lower, lower, higher, higher, otherKey, ineligible, legacy];
    const records = jsonRecords.map((json, index) => ({ id: index + 1, savedAt: `saved-${String(index)}`, json }));
    const factory = new IDBFactory();
    const databaseName = "pb-repair-score-keys";
    await seedPersonalBestDatabase(factory, databaseName, 4, records, []);
    const traced = tracedRustSelections();
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(factory, databaseName), undefined, traced.create);
    try {
      expect((await repository.list()).filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([5, 3]);
      const createdAfterRepair = traced.entries.length;
      await repository.list();
      expect(traced.entries).toHaveLength(createdAfterRepair);
      const tied = await repository.saveFrom({ export_flight_record_json: () => higher });
      expect(tied.personalBest).toMatchObject({ kind: "existing", id: 3 });
      const skipped = await repository.saveFrom({ export_flight_record_json: () => ineligible });
      expect(skipped.personalBest).toEqual({ kind: "ineligible" });
      expect((await repository.list()).filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([5, 3]);
      for (const record of records) expect(await repository.load(record.id)).toBe(record.json);
      expect(traced.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
    } finally {
      await deleteDatabase(factory, databaseName);
    }
  });

  it.each(["list", "save"] as const)("repairs an empty database through first %s", async (firstOperation) => {
    const factory = new IDBFactory();
    const databaseName = `pb-repair-empty-${firstOperation}`;
    const traced = tracedRustSelections();
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(factory, databaseName), undefined, traced.create);
    try {
      if (firstOperation === "save") {
        expect((await repository.saveFrom({ export_flight_record_json: interruptedRecord })).personalBest).toEqual({ kind: "ineligible" });
      } else await expect(repository.list()).resolves.toEqual([]);
      const state = await readPersonalBestDatabase(factory, databaseName);
      expect(state.personalBestIndexState).toEqual([{ key: currentPersonalBestIndexRevision }]);
      expect(state.personalBests).toEqual([]);
      const previousCount = traced.entries.length;
      await repository.list();
      expect(traced.entries).toHaveLength(previousCount);
      expect(traced.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
    } finally {
      await deleteDatabase(factory, databaseName);
    }
  });

  it("repairs old winners even when the first saved candidate is ineligible", async () => {
    const json = waterContactRecord();
    const factory = new IDBFactory();
    const databaseName = "pb-repair-ineligible-save";
    const bridge = new PersonalBestSelectionBridge(json);
    const key = bridge.key_hex();
    bridge.free();
    await seedPersonalBestDatabase(factory, databaseName, 4, [
      { id: 1, savedAt: "first", json }, { id: 2, savedAt: "second", json }
    ], [{ key, recordId: 2 }]);
    const repository = new FlightRecordRepository(
      new IndexedDbFlightRecordPersistence(factory, databaseName), undefined, (recordJson) => new PersonalBestSelectionBridge(recordJson)
    );
    try {
      expect((await repository.saveFrom({ export_flight_record_json: interruptedRecord })).personalBest).toEqual({ kind: "ineligible" });
      const state = await readPersonalBestDatabase(factory, databaseName);
      expect(state.personalBests).toEqual([{ key, recordId: 1 }]);
      expect(state.personalBestIndexState).toContainEqual({ key: currentPersonalBestIndexRevision });
    } finally {
      await deleteDatabase(factory, databaseName);
    }
  });

  it.each([
    ["list", "create", 1, 1], ["save", "create", 1, 1], ["list", "consider", 1, 1], ["save", "consider", 1, 1],
    ["list", "key", 1, 1], ["save", "key", 1, 1], ["list", "create", 2, 1], ["save", "create", 2, 1],
    ["list", "key", 2, 1], ["save", "key", 2, 1], ["list", "consider", 1, 2], ["save", "consider", 1, 2]
  ] as const)("rolls back first %s and frees Rust selections on %s failure at record %i / comparison %i", async (firstOperation, failure, recordIndex, consideration) => {
    const json = waterContactRecord();
    const factory = new IDBFactory();
    const databaseName = `pb-repair-failure-${firstOperation}-${failure}`;
    const bridge = new PersonalBestSelectionBridge(json);
    const key = bridge.key_hex();
    bridge.free();
    await seedPersonalBestDatabase(factory, databaseName, 4, [
      { id: 1, savedAt: "first", json }, { id: 2, savedAt: "second", json }
    ], [{ key, recordId: 2 }]);
    const before = await readPersonalBestDatabase(factory, databaseName);
    const traced = tracedRustSelections({ failure, creation: recordIndex + (firstOperation === "save" ? 1 : 0), consideration });
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(factory, databaseName), undefined, traced.create);
    try {
      await expect(firstOperation === "save" ? repository.saveFrom({ export_flight_record_json: () => json }) : repository.list()).rejects.toThrow();
      expect(await readPersonalBestDatabase(factory, databaseName)).toEqual(before);
      expect(traced.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
      expect((await repository.list()).filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([1]);
      expect(traced.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
    } finally {
      await deleteDatabase(factory, databaseName);
    }
  });

  it.each(["list", "save"] as const)("frees retained selections when first %s aborts during cursor traversal", async (firstOperation) => {
    const json = waterContactRecord();
    const factory = new IDBFactory();
    const databaseName = `pb-repair-cursor-abort-${firstOperation}`;
    const bridge = new PersonalBestSelectionBridge(json);
    const key = bridge.key_hex();
    bridge.free();
    await seedPersonalBestDatabase(factory, databaseName, 4, [
      { id: 1, savedAt: "first", json }, { id: 2, savedAt: "second", json }
    ], [{ key, recordId: 2 }]);
    const before = await readPersonalBestDatabase(factory, databaseName);
    const traced = tracedRustSelections();
    const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(factory, databaseName), undefined, traced.create);
    const originalContinue = Object.getOwnPropertyDescriptor(FakeCursor.prototype, "continue")?.value as IDBCursor["continue"];
    const cursorContinue = vi.spyOn(FakeCursor.prototype, "continue").mockImplementationOnce(function (this: IDBCursor, requestedKey?: IDBValidKey) {
      if (!(this.source instanceof FakeObjectStore)) throw new Error("Repair cursor must use the records store");
      this.source.add({ id: 1, savedAt: "duplicate", json });
      originalContinue.call(this, requestedKey);
    });
    try {
      await expect(firstOperation === "save" ? repository.saveFrom({ export_flight_record_json: () => json }) : repository.list()).rejects.toThrow();
    } finally {
      cursorContinue.mockRestore();
    }
    try {
      expect(traced.entries).toHaveLength(firstOperation === "save" ? 2 : 1);
      expect(traced.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
      expect(await readPersonalBestDatabase(factory, databaseName)).toEqual(before);
      expect((await repository.list()).filter(({ personalBest }) => personalBest).map(({ id }) => id)).toEqual([1]);
      expect(traced.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
    } finally {
      await deleteDatabase(factory, databaseName);
    }
  });

  it.each([
    ["list", "personalBests"], ["list", "personalBestIndexState"],
    ["save", "personalBests"], ["save", "personalBestIndexState"], ["save", "records"], ["save", "recordMetadata"]
  ] as const)(
    "rolls back repair and first %s on an asynchronous %s write failure", async (firstOperation, failedStore) => {
      const json = waterContactRecord();
      const factory = new IDBFactory();
      const databaseName = `pb-repair-write-failure-${failedStore}`;
      const bridge = new PersonalBestSelectionBridge(json);
      const key = bridge.key_hex();
      bridge.free();
      await seedPersonalBestDatabase(factory, databaseName, 4, [
        { id: 1, savedAt: "first", json }, { id: 2, savedAt: "second", json }
      ], [{ key, recordId: 2 }]);
      const before = await readPersonalBestDatabase(factory, databaseName);
      const traced = tracedRustSelections();
      const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(factory, databaseName), undefined, traced.create);
      const originalPut = Object.getOwnPropertyDescriptor(FakeObjectStore.prototype, "put")?.value as IDBObjectStore["put"];
      const originalAdd = Object.getOwnPropertyDescriptor(FakeObjectStore.prototype, "add")?.value as IDBObjectStore["add"];
      const put = vi.spyOn(FakeObjectStore.prototype, "put").mockImplementation(function (this: IDBObjectStore, value: unknown, requestedKey?: IDBValidKey) {
        if (this.name === failedStore) {
          originalAdd.call(this, value, requestedKey);
          return originalAdd.call(this, value, requestedKey);
        }
        return originalPut.call(this, value, requestedKey);
      });
      const add = vi.spyOn(FakeObjectStore.prototype, "add").mockImplementation(function (this: IDBObjectStore, value: unknown, requestedKey?: IDBValidKey) {
        return originalAdd.call(this, this.name === failedStore ? { ...value as Record<string, unknown>, id: 1 } : value, requestedKey);
      });
      try {
        await expect(firstOperation === "save" ? repository.saveFrom({ export_flight_record_json: () => json }) : repository.list()).rejects.toThrow();
      } finally {
        put.mockRestore();
        add.mockRestore();
      }
      try {
        expect(await readPersonalBestDatabase(factory, databaseName)).toEqual(before);
        expect(traced.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
        const saved = await repository.saveFrom({ export_flight_record_json: () => json });
        expect(saved.id).toBe(3);
        expect(saved.personalBest).toEqual({ kind: "existing", id: 1, key });
        expect(traced.entries.every(({ freeCount }) => freeCount === 1)).toBe(true);
      } finally {
        await deleteDatabase(factory, databaseName);
      }
    }
  );
});

const cachedWaterContactRecords = new Map<number, string>();

function tailWaterContactRecord(): string {
  waterContactRecord();
  const session = new HybridGameSessionBridge(0, 21, 22);
  try {
    session.open_setup();
    session.prepare();
    session.mark_briefing_ready();
    session.start_countdown(1);
    session.advance_countdown();
    session.launch();
    const neutral = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
      desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });
    for (let tick = 0; tick < 4_000 && session.phase_code() === 5; tick += 1) session.advance_tick_json(neutral);
    expect(session.phase_code()).toBe(7);
    const json = session.export_flight_record_json();
    expect(JSON.parse(json) as unknown).toMatchObject({ schema_version: 6,
      finalization: { reason: "water_contact", disposition: "complete", failure: null } });
    return json;
  } finally {
    session.free();
  }
}

function legacyArchiveRecord(json: string, version: 1 | 2 | 3 | 4): string {
  const record = JSON.parse(json) as { schema_version: number; header: { personal_best_key?: unknown;
    physics_model_version?: unknown; score_definition_version?: unknown; difficulty: { hud_profile?: unknown } } };
  record.schema_version = version;
  delete record.header.personal_best_key;
  if (version < 4) delete record.header.physics_model_version;
  if (version < 3) delete record.header.score_definition_version;
  if (version < 2) delete record.header.difficulty.hud_profile;
  return JSON.stringify(record);
}

function waterContactRecord(pitch = 0): string {
  const cached = cachedWaterContactRecords.get(pitch);
  if (cached !== undefined) return cached;
  if (cachedWaterContactRecords.size === 0) {
    const wasmPath = fileURLToPath(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url));
    initSync({ module: new Uint8Array(readFileSync(wasmPath)) });
  }
  const session = new GameSessionBridge(0);
  try {
    session.open_setup();
    session.prepare();
    session.mark_briefing_ready();
    session.start_countdown(1);
    session.advance_countdown();
    let snapshot = parseFlightSnapshot(session.launch());
    for (let tick = 0; tick < 3_000 && snapshot.terminal === "airborne"; tick++) {
      snapshot = parseFlightSnapshot(session.advance_tick(pitch, 0, 0, 0));
    }
    expect(snapshot.terminal).toBe("water-contact");
    const json = session.export_flight_record_json();
    cachedWaterContactRecords.set(pitch, json);
    return json;
  } finally {
    session.free();
  }
}

function interruptedRecord(): string {
  waterContactRecord();
  const session = new GameSessionBridge(0);
  try {
    session.open_setup();
    session.prepare();
    session.mark_briefing_ready();
    session.start_countdown(1);
    session.advance_countdown();
    session.launch();
    session.abort();
    return session.export_flight_record_json();
  } finally {
    session.free();
  }
}

function tracedRustSelections(injected?: {
  readonly failure: "create" | "consider" | "key"; readonly creation: number; readonly consideration: number
}) {
  const entries: { freeCount: number }[] = [];
  let creations = 0;
  return {
    entries,
    create: (json: string): PersonalBestSelectionPort => {
      creations++;
      const failure = creations === injected?.creation ? injected.failure : null;
      if (failure === "create") throw new Error("Injected selection creation failure");
      const selection = new PersonalBestSelectionBridge(json);
      const trace = { freeCount: 0 };
      let considerations = 0;
      entries.push(trace);
      return {
        candidate_is_best: () => selection.candidate_is_best(),
        consider_existing: (id, recordJson) => {
          considerations++;
          if (failure === "consider" && considerations === injected?.consideration) throw new Error("Injected selection comparison failure");
          selection.consider_existing(id, recordJson);
        },
        free: () => { trace.freeCount++; selection.free(); },
        is_eligible: () => selection.is_eligible(),
        key_hex: () => failure === "key" ? "invalid" : selection.key_hex(),
        selected_existing_id: () => selection.selected_existing_id()
      };
    }
  };
}

async function readPersonalBestDatabase(factory: IDBFactory, databaseName: string): Promise<Record<string, unknown[]>> {
  const database = await openTestDatabase(factory, databaseName);
  try {
    const transaction = database.transaction([...database.objectStoreNames], "readonly");
    const entries = await Promise.all([...database.objectStoreNames].map(async (name) => [
      name, await requestResult(transaction.objectStore(name).getAll()) as unknown[]
    ] as const));
    return Object.fromEntries(entries);
  } finally {
    database.close();
  }
}

async function seedPersonalBestDatabase(
  factory: IDBFactory, databaseName: string, version: 3 | 4,
  records: readonly StoredFlightRecord[], personalBests: readonly { readonly key: string; readonly recordId: number }[], legacyMarker = true
): Promise<void> {
  const request = factory.open(databaseName, version);
  request.onupgradeneeded = () => {
    const database = request.result;
    database.createObjectStore("records", { keyPath: "id", autoIncrement: true });
    database.createObjectStore("recordMetadata", { keyPath: "id" });
    database.createObjectStore("personalBests", { keyPath: "key" });
    if (version === 4) database.createObjectStore("personalBestIndexState", { keyPath: "key" });
  };
  const database = await requestResult(request);
  try {
    const transaction = database.transaction([...database.objectStoreNames], "readwrite");
    for (const record of records) {
      transaction.objectStore("records").put(record);
      transaction.objectStore("recordMetadata").put({ id: record.id, savedAt: record.savedAt });
    }
    for (const entry of personalBests) transaction.objectStore("personalBests").put(entry);
    if (version === 4 && legacyMarker) transaction.objectStore("personalBestIndexState").put({ key: "canonical-v1" });
    await new Promise<void>((resolve, reject) => {
      transaction.oncomplete = () => { resolve(); };
      transaction.onabort = () => { reject(transaction.error ?? new Error("Personal Best seed aborted")); };
    });
  } finally {
    database.close();
  }
}

function createIneligibleSelection(): PersonalBestSelectionPort {
  return new TestPersonalBestSelection(false, false);
}

class TestPersonalBestSelection implements PersonalBestSelectionPort {
  readonly consideredIds: number[] = [];
  freed = false;
  private existingId: number | null = null;

  constructor(
    private readonly candidateWinsAfterScan = false,
    private readonly eligible = true
  ) {}

  candidate_is_best(): boolean {
    return this.existingId === null || this.candidateWinsAfterScan;
  }

  consider_existing(id: number): void {
    this.consideredIds.push(id);
    this.existingId ??= id;
  }

  free(): void {
    this.freed = true;
  }

  is_eligible(): boolean {
    return this.eligible;
  }

  key_hex(): string {
    return "a".repeat(64);
  }

  selected_existing_id(): number {
    return this.existingId ?? 0;
  }
}

async function seedVersionedDatabase(
  factory: IDBFactory,
  databaseName: string,
  version: number,
  records: readonly StoredFlightRecord[],
  metadata: readonly { readonly id: number; readonly savedAt: string }[] = records.map(({ id, savedAt }) => ({ id, savedAt }))
): Promise<void> {
  const database = await new Promise<IDBDatabase>((resolve, reject) => {
    const request = factory.open(databaseName, version);
    request.onupgradeneeded = () => {
      request.result.createObjectStore("records", { keyPath: "id", autoIncrement: true });
      if (version >= 2) request.result.createObjectStore("recordMetadata", { keyPath: "id" });
    };
    request.onsuccess = () => { resolve(request.result); };
    request.onerror = () => { reject(request.error ?? new Error("Failed to seed IndexedDB")); };
  });
  const storeNames = version >= 2 ? ["records", "recordMetadata"] : ["records"];
  const transaction = database.transaction(storeNames, "readwrite");
  for (const record of records) transaction.objectStore("records").put(record);
  if (version >= 2) {
    for (const entry of metadata) transaction.objectStore("recordMetadata").put(entry);
  }
  await new Promise<void>((resolve, reject) => {
    transaction.oncomplete = () => { resolve(); };
    transaction.onabort = () => { reject(transaction.error ?? new Error("Failed to seed IndexedDB")); };
    transaction.onerror = () => { reject(transaction.error ?? new Error("Failed to seed IndexedDB")); };
  });
  database.close();
}

async function openTestDatabase(factory: IDBFactory, databaseName: string): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = factory.open(databaseName);
    request.onsuccess = () => { resolve(request.result); };
    request.onerror = () => { reject(request.error ?? new Error("Failed to open IndexedDB")); };
  });
}

async function readDatabaseVersion(factory: IDBFactory, databaseName: string): Promise<number> {
  const database = await openTestDatabase(factory, databaseName);
  try {
    return database.version;
  } finally {
    database.close();
  }
}

function requestResult<T>(request: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => { resolve(request.result); };
    request.onerror = () => { reject(request.error ?? new Error("IndexedDB request failed")); };
  });
}

async function deleteDatabase(factory: IDBFactory, databaseName: string): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const request = factory.deleteDatabase(databaseName);
    request.onsuccess = () => { resolve(); };
    request.onerror = () => { reject(request.error ?? new Error("Failed to delete IndexedDB")); };
  });
}

function memoryIndexedDb(initialRecords: readonly StoredFlightRecord[]) {
  type FakeRequest = {
    result: unknown;
    error: DOMException | null;
    onsuccess: ((event: Event) => void) | null;
    onerror: ((event: Event) => void) | null;
  };
  const records = [...initialRecords];
  const metadata = initialRecords.map(({ id, savedAt }) => ({ id, savedAt }));
  const personalBests = new Map<string, { readonly key: string; readonly recordId: number }>();
  let personalBestIndexInitialized = false;
  let nextId = Math.max(0, ...records.map(({ id }) => id)) + 1;
  let cursorCount = 0;

  const database = {
    objectStoreNames: { contains: () => true },
    onversionchange: null as ((event: IDBVersionChangeEvent) => void) | null,
    transaction: () => {
      let pending = 0;
      let completionScheduled = false;
      let completed = false;
      const transaction = {
        addEventListener: () => undefined,
        oncomplete: null as ((event: Event) => void) | null,
        onabort: null as ((event: Event) => void) | null,
        onerror: null as ((event: Event) => void) | null,
        abort: () => { throw new Error("Unexpected transaction abort in test"); },
        objectStore: (name: string) => {
          const request = (action: () => unknown): FakeRequest => {
            const result: FakeRequest = { result: undefined, error: null, onsuccess: null, onerror: null };
            pending += 1;
            queueMicrotask(() => {
              result.result = action();
              result.onsuccess?.(new Event("success"));
              pending -= 1;
              scheduleCompletion();
            });
            return result;
          };
          return {
            add: (value: { readonly savedAt: string; readonly json: string; readonly id?: number }) => request(() => {
              if (name === "records") {
                const id = nextId++;
                records.push({ id, savedAt: value.savedAt, json: value.json });
                return id;
              }
              if (name === "recordMetadata") {
                metadata.push({ id: value.id ?? 0, savedAt: value.savedAt });
                return value.id;
              }
              throw new Error(`Unexpected add to ${name}`);
            }),
            get: (key: string | number) => request(() => name === "records"
              ? records.find((record) => record.id === key)
              : name === "personalBestIndexState"
                ? personalBestIndexInitialized && key === currentPersonalBestIndexRevision ? { key } : undefined
                : personalBests.get(String(key))),
            getAll: () => request(() => name === "records"
              ? [...records]
              : name === "recordMetadata" ? [...metadata] : [...personalBests.values()]),
            put: (value: { readonly key: string; readonly recordId?: number }) => request(() => {
              if (name === "personalBests" && value.recordId !== undefined) {
                personalBests.set(value.key, { key: value.key, recordId: value.recordId });
              } else if (name === "personalBestIndexState" && value.key === currentPersonalBestIndexRevision) {
                personalBestIndexInitialized = true;
              } else throw new Error(`Unexpected put to ${name}`);
              return value.key;
            }),
            clear: () => request(() => {
              if (name !== "personalBests") throw new Error(`Unexpected clear of ${name}`);
              personalBests.clear();
            }),
            openCursor: () => {
              cursorCount += 1;
              const cursorRequest: FakeRequest & { onsuccess: ((event: Event) => void) | null } = {
                result: undefined,
                error: null,
                onsuccess: null,
                onerror: null
              };
              let index = 0;
              const advance = (): void => {
                pending += 1;
                queueMicrotask(() => {
                  const record = records[index];
                  cursorRequest.result = record === undefined ? null : {
                    primaryKey: record.id,
                    value: record,
                    continue: () => { index += 1; advance(); }
                  };
                  cursorRequest.onsuccess?.(new Event("success"));
                  pending -= 1;
                  scheduleCompletion();
                });
              };
              advance();
              return cursorRequest;
            }
          };
        }
      };
      const scheduleCompletion = (): void => {
        if (pending !== 0 || completionScheduled || completed) return;
        completionScheduled = true;
        queueMicrotask(() => {
          completionScheduled = false;
          if (pending !== 0 || completed) return;
          completed = true;
          transaction.oncomplete?.(new Event("complete"));
        });
      };
      return transaction;
    },
    close: () => undefined
  };
  const factory = {
    open: () => {
      const request: {
        result: typeof database;
        error: DOMException | null;
        onsuccess: ((event: Event) => void) | null;
        onerror: ((event: Event) => void) | null;
        onblocked: ((event: Event) => void) | null;
        onupgradeneeded: ((event: IDBVersionChangeEvent) => void) | null;
      } = {
        result: database,
        error: null,
        onsuccess: null,
        onerror: null,
        onblocked: null,
        onupgradeneeded: null
      };
      queueMicrotask(() => request.onsuccess?.(new Event("success")));
      return request as unknown as IDBOpenDBRequest;
    }
  } as unknown as IDBFactory;
  return {
    factory,
    records: () => records,
    metadata: () => metadata,
    personalBests,
    cursorCount: () => cursorCount,
    personalBestIndexInitialized: () => personalBestIndexInitialized
  };
}

function indexedDbFactoryWithReadResult(storeName: string, result: unknown, personalBests: unknown = []): IDBFactory {
  const makeRequest = (requestResult: unknown) => {
    const request: {
    result: unknown;
    error: DOMException | null;
    onsuccess: ((event: Event) => void) | null;
    onerror: ((event: Event) => void) | null;
    } = { result: requestResult, error: null, onsuccess: null, onerror: null };
    queueMicrotask(() => request.onsuccess?.(new Event("success")));
    return request;
  };
  const transaction: {
    oncomplete: ((event: Event) => void) | null;
    onabort: ((event: Event) => void) | null;
    onerror: ((event: Event) => void) | null;
    abort: () => void;
    objectStore: (name: string) => {
      get: (key: number) => ReturnType<typeof makeRequest>;
      getAll: () => ReturnType<typeof makeRequest>;
    };
  } = {
    oncomplete: null,
    onabort: null,
    onerror: null,
    abort: () => {
      queueMicrotask(() => transaction.onabort?.(new Event("abort")));
    },
    objectStore: (name) => ({
      get: () => makeRequest(name === "personalBestIndexState" ? { key: currentPersonalBestIndexRevision } : name === storeName ? result : undefined),
      getAll: () => makeRequest(name === "personalBests" ? personalBests : result)
    })
  };
  const database = {
    objectStoreNames: { contains: () => true },
    transaction: (storeNameArg: string | string[]) => {
      if (!(Array.isArray(storeNameArg) ? storeNameArg.includes(storeName) : storeNameArg === storeName)) {
        throw new Error(`Unexpected object store ${String(storeNameArg)}`);
      }
      setTimeout(() => transaction.oncomplete?.(new Event("complete")), 0);
      return transaction;
    },
    close: () => undefined
  };
  const openRequest: {
    result: typeof database;
    error: DOMException | null;
    onsuccess: ((event: Event) => void) | null;
    onerror: ((event: Event) => void) | null;
    onblocked: ((event: Event) => void) | null;
    onupgradeneeded: ((event: IDBVersionChangeEvent) => void) | null;
  } = {
    result: database,
    error: null,
    onsuccess: null,
    onerror: null,
    onblocked: null,
    onupgradeneeded: null
  };
  return {
    open: () => {
      queueMicrotask(() => { openRequest.onsuccess?.(new Event("success")); });
      return openRequest as unknown as IDBOpenDBRequest;
    }
  } as unknown as IDBFactory;
}
