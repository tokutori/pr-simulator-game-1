import { describe, expect, it } from "vitest";
import {
  FlightRecordRepository,
  IndexedDbFlightRecordPersistence,
  type FlightRecordPersistencePort,
  type PersonalBestSelectionPort,
  type StoredFlightRecord,
  type StoredFlightRecordSummary
} from "../../web/src/game/flight-record-store.js";

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
    const repository = new FlightRecordRepository(persistence, () => new Date("2026-09-28T00:00:00.000Z"));
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
    const repository = new FlightRecordRepository(persistence);

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
    expect(await persistence.add(finalizedRecord, "2026-09-30T00:00:00.000Z", firstSelection)).toBe(2);
    expect(firstSelection.consideredIds).toEqual([1]);
    expect(database.personalBests.get("a".repeat(64))).toEqual({ key: "a".repeat(64), recordId: 2 });

    const secondSelection = new TestPersonalBestSelection(false);
    expect(await persistence.add(finalizedRecord, "2026-10-01T00:00:00.000Z", secondSelection)).toBe(3);
    expect(secondSelection.consideredIds).toEqual([2]);
    expect(database.cursorCount()).toBe(1);
    expect(database.records()).toHaveLength(3);
    expect(database.metadata()).toHaveLength(3);
    expect(database.personalBests.get("a".repeat(64))?.recordId).toBe(2);
  });

  it.each([
    "not-json",
    JSON.stringify({ schema_version: 2, header: [], samples: [{}], finalization: {} }),
    JSON.stringify({ schema_version: 1, header: [], samples: [{}], finalization: {} }),
    JSON.stringify({ schema_version: 1, header: {}, samples: [], finalization: {} }),
    JSON.stringify({ schema_version: 1, header: {}, samples: [{}], finalization: null })
  ])("rejects invalid or incomplete documents before storage", async (json) => {
    const persistence = new MemoryFlightRecordPersistence();
    const repository = new FlightRecordRepository(persistence);
    await expect(repository.saveFrom({ export_flight_record_json: () => json })).rejects.toThrow();
    expect(await repository.list()).toEqual([]);
  });

  it("rejects documents above the Rust format size limit", async () => {
    const persistence = new MemoryFlightRecordPersistence();
    const repository = new FlightRecordRepository(persistence);
    const oversized = `${finalizedRecord}${" ".repeat(16 * 1024 * 1024)}`;
    await expect(repository.saveFrom({ export_flight_record_json: () => oversized })).rejects.toThrow(RangeError);
    expect(await repository.list()).toEqual([]);
  });

  it("rejects invalid record identifiers", async () => {
    const repository = new FlightRecordRepository(new MemoryFlightRecordPersistence());
    await expect(repository.load(0)).rejects.toThrow(RangeError);
  });

  it("returns IndexedDB metadata when valid and reports malformed rows", async () => {
    const valid = new IndexedDbFlightRecordPersistence(indexedDbFactoryWithReadResult("recordMetadata", [
      { id: 3, savedAt: "2026-09-30T00:00:00.000Z" }
    ]));
    await expect(valid.getAll()).resolves.toEqual([
      { id: 3, savedAt: "2026-09-30T00:00:00.000Z", personalBest: false }
    ]);

    const personalBest = new IndexedDbFlightRecordPersistence(indexedDbFactoryWithReadResult(
      "recordMetadata",
      [{ id: 3, savedAt: "2026-09-30T00:00:00.000Z" }],
      [{ key: "a".repeat(64), recordId: 3 }]
    ));
    await expect(personalBest.getAll()).resolves.toEqual([
      { id: 3, savedAt: "2026-09-30T00:00:00.000Z", personalBest: true }
    ]);

    const malformed = new IndexedDbFlightRecordPersistence(indexedDbFactoryWithReadResult("recordMetadata", [
      { id: 3, savedAt: "2026-09-30T00:00:00.000Z" },
      { id: "invalid", savedAt: "2026-09-30T00:00:00.000Z" }
    ]));
    await expect(malformed.getAll()).rejects.toThrow("IndexedDB record metadata or Personal Best index is malformed");

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

class TestPersonalBestSelection implements PersonalBestSelectionPort {
  readonly consideredIds: number[] = [];
  freed = false;
  private existingId: number | null = null;

  constructor(private readonly candidateWinsAfterScan = false) {}

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
    return true;
  }

  key_hex(): string {
    return "a".repeat(64);
  }

  selected_existing_id(): number {
    return this.existingId ?? 0;
  }
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
              : personalBests.get(String(key))),
            getAll: () => request(() => name === "records"
              ? [...records]
              : name === "recordMetadata" ? [...metadata] : [...personalBests.values()]),
            put: (value: { readonly key: string; readonly recordId: number }) => request(() => {
              if (name !== "personalBests") throw new Error(`Unexpected put to ${name}`);
              personalBests.set(value.key, value);
              return value.key;
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
    cursorCount: () => cursorCount
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
    objectStore: (name: string) => {
      get: (key: number) => ReturnType<typeof makeRequest>;
      getAll: () => ReturnType<typeof makeRequest>;
    };
  } = {
    oncomplete: null,
    onabort: null,
    onerror: null,
    objectStore: (name) => ({
      get: () => makeRequest(name === storeName ? result : undefined),
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
