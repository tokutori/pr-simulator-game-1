import { describe, expect, it } from "vitest";
import {
  FlightRecordRepository,
  IndexedDbFlightRecordPersistence,
  type FlightRecordPersistencePort,
  type StoredFlightRecord,
  type StoredFlightRecordSummary
} from "../../web/src/game/flight-record-store.js";

class MemoryFlightRecordPersistence implements FlightRecordPersistencePort {
  private readonly records: StoredFlightRecord[] = [];

  add(json: string, savedAt: string): Promise<number> {
    const id = this.records.length + 1;
    this.records.push(Object.freeze({ id, savedAt, json }));
    return Promise.resolve(id);
  }

  get(id: number): Promise<StoredFlightRecord | null> {
    return Promise.resolve(this.records.find((record) => record.id === id) ?? null);
  }

  getAll(): Promise<readonly StoredFlightRecordSummary[]> {
    return Promise.resolve(this.records.map(({ id, savedAt }) => Object.freeze({ id, savedAt })));
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
    expect(entries[0]).toEqual({ id: 2, savedAt: "2026-09-28T00:00:00.000Z" });
    expect(entries[0]).not.toHaveProperty("json");
  });

  it("persists current schema records without coupling the browser adapter to a schema version", async () => {
    const persistence = new MemoryFlightRecordPersistence();
    const repository = new FlightRecordRepository(persistence);

    const saved = await repository.saveFrom({ export_flight_record_json: () => finalizedCustomHudRecord });

    expect(saved.json).toBe(finalizedCustomHudRecord);
    expect(await repository.load(saved.id)).toBe(finalizedCustomHudRecord);
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
    await expect(valid.getAll()).resolves.toEqual([{ id: 3, savedAt: "2026-09-30T00:00:00.000Z" }]);

    const malformed = new IndexedDbFlightRecordPersistence(indexedDbFactoryWithReadResult("recordMetadata", [
      { id: 3, savedAt: "2026-09-30T00:00:00.000Z" },
      { id: "invalid", savedAt: "2026-09-30T00:00:00.000Z" }
    ]));
    await expect(malformed.getAll()).rejects.toThrow("IndexedDB record metadata is malformed");

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

function indexedDbFactoryWithReadResult(storeName: string, result: unknown): IDBFactory {
  const metadataRequest: {
    result: unknown;
    error: DOMException | null;
    onsuccess: ((event: Event) => void) | null;
    onerror: ((event: Event) => void) | null;
  } = { result, error: null, onsuccess: null, onerror: null };
  const transaction: {
    oncomplete: ((event: Event) => void) | null;
    onabort: ((event: Event) => void) | null;
    onerror: ((event: Event) => void) | null;
    objectStore: (name: string) => {
      get: (key: number) => typeof metadataRequest;
      getAll: () => typeof metadataRequest;
    };
  } = {
    oncomplete: null,
    onabort: null,
    onerror: null,
    objectStore: () => ({
      get: () => metadataRequest,
      getAll: () => metadataRequest
    })
  };
  const database = {
    objectStoreNames: { contains: () => true },
    transaction: (storeNameArg: string) => {
      if (storeNameArg !== storeName) throw new Error(`Unexpected object store ${storeNameArg}`);
      queueMicrotask(() => {
        metadataRequest.onsuccess?.(new Event("success"));
        transaction.oncomplete?.(new Event("complete"));
      });
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
