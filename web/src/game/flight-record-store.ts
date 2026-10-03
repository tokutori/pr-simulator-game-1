export interface FlightRecordExportPort {
  export_flight_record_json(): string;
}

export interface StoredFlightRecord {
  readonly id: number;
  readonly savedAt: string;
  readonly json: string;
}

export interface StoredFlightRecordSummary {
  readonly id: number;
  readonly savedAt: string;
  readonly personalBest: boolean;
}

export type PersonalBestUpdate =
  | { readonly kind: "not-evaluated" }
  | { readonly kind: "ineligible" }
  | { readonly kind: "candidate"; readonly id: number; readonly key: string }
  | { readonly kind: "existing"; readonly id: number; readonly key: string };

export interface PersonalBestSelectionPort {
  candidate_is_best(): boolean;
  consider_existing(id: number, json: string): void;
  free(): void;
  is_eligible(): boolean;
  key_hex(): string;
  selected_existing_id(): number;
}

export type PersonalBestSelectionFactory = (json: string) => PersonalBestSelectionPort;

export interface SavedFlightRecord extends StoredFlightRecord {
  readonly personalBest: PersonalBestUpdate;
}

export interface FlightRecordPersistencePort {
  add(json: string, savedAt: string, selection: PersonalBestSelectionPort, createSelection: PersonalBestSelectionFactory): Promise<number>;
  get(id: number): Promise<StoredFlightRecord | null>;
  getAll(createSelection: PersonalBestSelectionFactory): Promise<readonly StoredFlightRecordSummary[]>;
}

const maximumRecordBytes = 16 * 1024 * 1024;
// Change with the Rust physics model version so persisted eligibility is re-evaluated.
const personalBestIndexRevision = "first-winner-physics-v2";

export class FlightRecordRepository {
  constructor(
    private readonly persistence: FlightRecordPersistencePort,
    private readonly now: () => Date = () => new Date(),
    private readonly createSelection: PersonalBestSelectionFactory
  ) {}

  async saveFrom(session: FlightRecordExportPort): Promise<SavedFlightRecord> {
    const json = session.export_flight_record_json();
    validateFinalizedRecord(json);
    const savedAt = this.now().toISOString();
    const selection = this.createSelection(json);
    try {
      const id = await this.persistence.add(json, savedAt, selection, this.createSelection);
      return Object.freeze({ id, savedAt, json, personalBest: resolvePersonalBestUpdate(id, selection) });
    } finally {
      selection.free();
    }
  }

  async load(id: number): Promise<string | null> {
    if (!Number.isSafeInteger(id) || id < 1) throw new RangeError("Record identifier must be a positive integer");
    return (await this.persistence.get(id))?.json ?? null;
  }

  async list(): Promise<readonly StoredFlightRecordSummary[]> {
    const records = await this.persistence.getAll(this.createSelection);
    return Object.freeze([...records]
      .sort((left, right) => right.id - left.id)
      .map(({ id, savedAt, personalBest }) => Object.freeze({ id, savedAt, personalBest })));
  }
}

export class IndexedDbFlightRecordPersistence implements FlightRecordPersistencePort {
  constructor(
    private readonly factory: IDBFactory,
    private readonly databaseName = "birdman-flight-records"
  ) {}

  async add(
    json: string, savedAt: string, selection: PersonalBestSelectionPort, createSelection: PersonalBestSelectionFactory
  ): Promise<number> {
    return this.withDatabase((database) => new Promise<number>((resolve, reject) => {
      const transaction = database.transaction(["records", "recordMetadata", "personalBests", "personalBestIndexState"], "readwrite");
      const records = transaction.objectStore("records");
      let id: number | null = null;
      let failure: { readonly kind: "unrecorded" } | { readonly kind: "recorded"; readonly error: Error } = { kind: "unrecorded" };
      const abort = (error: unknown): void => {
        failure = { kind: "recorded", error: normalizeInsertionError(error) };
        transaction.abort();
      };
      const addCandidate = (): void => {
        const request = records.add({ savedAt, json });
        request.onsuccess = () => {
          try {
            id = Number(request.result);
            transaction.objectStore("recordMetadata").add({ id, savedAt });
            if (selection.is_eligible()) {
              const key = selection.key_hex();
              const selectedId = selection.candidate_is_best() ? id : selection.selected_existing_id();
              if (key.length !== 64 || !Number.isSafeInteger(selectedId) || selectedId < 1) {
                abort(new TypeError("Rust returned an invalid Personal Best selection"));
                return;
              }
              transaction.objectStore("personalBests").put({ key, recordId: selectedId });
            }
          } catch (error: unknown) {
            abort(error);
          }
        };
      };
      const addWithPersonalBest = (): void => {
        if (selection.is_eligible()) {
          const key = selection.key_hex();
          const scanExistingRecords = (): void => {
            const cursorRequest = records.openCursor();
            cursorRequest.onsuccess = () => {
              const cursor = cursorRequest.result;
              if (cursor === null) {
                addCandidate();
                return;
              }
              const value: unknown = cursor.value;
              if (!isStoredFlightRecord(value)) {
                abort(new TypeError("IndexedDB flight record is malformed"));
                return;
              }
              try {
                selection.consider_existing(Number(cursor.primaryKey), value.json);
              } catch (error: unknown) {
                abort(error);
                return;
              }
              cursor.continue();
            };
          };
          const indexedRequest = transaction.objectStore("personalBests").get(key);
          indexedRequest.onsuccess = () => {
            const indexed: unknown = indexedRequest.result;
            if (indexed === undefined) {
              scanExistingRecords();
              return;
            }
            if (!isPersonalBestEntry(indexed) || indexed.key !== key) {
              abort(new TypeError("IndexedDB Personal Best index is malformed"));
              return;
            }
            const existingRequest = records.get(indexed.recordId);
            existingRequest.onsuccess = () => {
              const existing: unknown = existingRequest.result;
              if (!isStoredFlightRecord(existing)) {
                abort(new TypeError("IndexedDB Personal Best record is missing or malformed"));
                return;
              }
              try {
                selection.consider_existing(indexed.recordId, existing.json);
              } catch (error: unknown) {
                abort(error);
                return;
              }
              addCandidate();
            };
          };
        } else {
          addCandidate();
        }
      };
      preparePersonalBestIndex(transaction, createSelection, addWithPersonalBest, abort);
      transaction.oncomplete = () => {
        if (id === null) reject(new Error("IndexedDB completed without a record key"));
        else resolve(id);
      };
      transaction.onabort = () => {
        const error = failure.kind === "recorded"
          ? failure.error
          : transaction.error ?? new Error("IndexedDB record insertion was aborted");
        reject(error);
      };
    }));
  }

  async get(id: number): Promise<StoredFlightRecord | null> {
    return this.withDatabase((database) => new Promise<StoredFlightRecord | null>((resolve, reject) => {
      const transaction = database.transaction("records", "readonly");
      const request = transaction.objectStore("records").get(id);
      let record: StoredFlightRecord | null = null;
      let malformed = false;
      request.onsuccess = () => {
        const result: unknown = request.result;
        if (result === undefined) return;
        if (isStoredFlightRecord(result)) record = result;
        else malformed = true;
      };
      request.onerror = () => { reject(request.error ?? new Error("IndexedDB record read failed")); };
      transaction.oncomplete = () => {
        if (malformed) reject(new TypeError("IndexedDB flight record is malformed"));
        else resolve(record);
      };
      transaction.onabort = () => { reject(transaction.error ?? new Error("IndexedDB record read was aborted")); };
      transaction.onerror = () => { reject(transaction.error ?? new Error("IndexedDB record read failed")); };
    }));
  }

  async getAll(
    createSelection: PersonalBestSelectionFactory
  ): Promise<readonly StoredFlightRecordSummary[]> {
    return this.withDatabase((database) => new Promise<readonly StoredFlightRecordSummary[]>((resolve, reject) => {
      const transaction = database.transaction(["records", "recordMetadata", "personalBests", "personalBestIndexState"], "readwrite");
      const metadataRequest = transaction.objectStore("recordMetadata").getAll();
      const personalBests = transaction.objectStore("personalBests");
      let metadata: readonly StoredFlightRecordMetadata[] | null = null;
      let personalBestRecordIds: ReadonlySet<number> | null = null;
      let failure: Error | null = null;
      const abort = (error: unknown): void => {
        failure = error instanceof Error ? error : new Error(String(error));
        transaction.abort();
      };
      const readPersonalBestIndex = (): void => {
        const personalBestRequest = personalBests.getAll();
        personalBestRequest.onsuccess = () => {
          const result: unknown = personalBestRequest.result;
          if (Array.isArray(result) && result.every(isPersonalBestEntry)) {
            personalBestRecordIds = new Set(result.map(({ recordId }) => recordId));
          } else abort(new TypeError("IndexedDB Personal Best index is malformed"));
        };
      };
      metadataRequest.onsuccess = () => {
        const result: unknown = metadataRequest.result;
        if (!Array.isArray(result) || !result.every(isStoredFlightRecordMetadata)) {
          abort(new TypeError("IndexedDB record metadata is malformed"));
          return;
        }
        metadata = result;
        preparePersonalBestIndex(transaction, createSelection, readPersonalBestIndex, abort);
      };
      transaction.oncomplete = () => {
        const currentMetadata = metadata;
        const currentPersonalBestRecordIds = personalBestRecordIds;
        if (currentMetadata === null || currentPersonalBestRecordIds === null) {
          reject(new TypeError("IndexedDB record metadata or Personal Best index is malformed"));
        } else {
          resolve(Object.freeze(currentMetadata.map(({ id, savedAt }) => Object.freeze({
            id,
            savedAt,
            personalBest: currentPersonalBestRecordIds.has(id)
          }))));
        }
      };
      transaction.onabort = () => { reject(failure ?? transaction.error ?? new Error("IndexedDB record list was aborted")); };
    }));
  }

  private async withDatabase<T>(operation: (database: IDBDatabase) => Promise<T>): Promise<T> {
    const database = await openDatabase(this.factory, this.databaseName);
    try {
      return await operation(database);
    } finally {
      database.close();
    }
  }
}

function normalizeInsertionError(reason: unknown): Error {
  try {
    return reason instanceof Error ? reason : new Error(String(reason), { cause: reason });
  } catch {
    return new Error("IndexedDB record insertion failed with an unprintable cause", { cause: reason });
  }
}

function preparePersonalBestIndex(
  transaction: IDBTransaction,
  createSelection: PersonalBestSelectionFactory,
  completed: () => void,
  abort: (error: unknown) => void
): void {
  const stateStore = transaction.objectStore("personalBestIndexState");
  const stateRequest = stateStore.get(personalBestIndexRevision);
  stateRequest.onsuccess = () => {
    try {
      const state: unknown = stateRequest.result;
      if (state === undefined) {
        rebuildPersonalBestIndex(transaction, transaction.objectStore("personalBests"), stateStore, createSelection, completed, abort);
      } else if (!isPersonalBestIndexState(state)) {
        abort(new TypeError("IndexedDB Personal Best index state is malformed"));
      } else completed();
    } catch (error: unknown) {
      abort(error);
    }
  };
}

function rebuildPersonalBestIndex(
  transaction: IDBTransaction,
  personalBests: IDBObjectStore,
  stateStore: IDBObjectStore,
  createSelection: PersonalBestSelectionFactory,
  completed: () => void,
  abort: (error: unknown) => void
): void {
  const records = transaction.objectStore("records");
  const selections = new Map<string, PersonalBestSelectionPort>();
  let activeSelection: PersonalBestSelectionPort | null = null;
  const releaseSelections = (): void => {
    activeSelection?.free();
    activeSelection = null;
    for (const selection of selections.values()) selection.free();
    selections.clear();
  };
  transaction.addEventListener("abort", releaseSelections, { once: true });
  const fail = (error: unknown): void => {
    releaseSelections();
    abort(error);
  };
  const clearRequest = personalBests.clear();
  clearRequest.onsuccess = () => {
    const cursorRequest = records.openCursor();
    cursorRequest.onsuccess = () => {
      const cursor = cursorRequest.result;
      if (cursor === null) {
        const selected: { readonly key: string; readonly recordId: number }[] = [];
        try {
          for (const [key, selection] of selections) {
            const recordId = selection.selected_existing_id();
            if (key.length !== 64 || !/^[0-9a-f]{64}$/.test(key) || !Number.isSafeInteger(recordId) || recordId < 1) {
              fail(new TypeError("Rust returned an invalid Personal Best migration result"));
              return;
            }
            selected.push({ key, recordId });
          }
          for (const entry of selected) personalBests.put(entry);
          stateStore.put({ key: personalBestIndexRevision });
        } catch (error: unknown) {
          fail(error);
          return;
        }
        releaseSelections();
        try {
          completed();
        } catch (error: unknown) {
          abort(error);
        }
        return;
      }
      const value: unknown = cursor.value;
      if (!isStoredFlightRecord(value)) {
        fail(new TypeError("IndexedDB flight record is malformed during Personal Best migration"));
        return;
      }
      const id = Number(cursor.primaryKey);
      try {
        activeSelection = createSelection(value.json);
        if (!activeSelection.is_eligible()) {
          activeSelection.free();
          activeSelection = null;
          cursor.continue();
          return;
        }
        const key = activeSelection.key_hex();
        if (key.length !== 64 || !/^[0-9a-f]{64}$/.test(key)) {
          fail(new TypeError("Rust returned an invalid Personal Best key during migration"));
          return;
        }
        const existing = selections.get(key);
        if (existing === undefined) {
          activeSelection.consider_existing(id, value.json);
          selections.set(key, activeSelection);
          activeSelection = null;
        } else {
          existing.consider_existing(id, value.json);
          activeSelection.free();
          activeSelection = null;
        }
      } catch (error: unknown) {
        fail(error);
        return;
      }
      cursor.continue();
    };
  };
}

function openDatabase(factory: IDBFactory, databaseName: string): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    let ownership: "pending" | "abandoned" | "transferred" | "closed" = "pending";
    const abandon = (error: Error): void => {
      if (ownership !== "pending") return;
      ownership = "abandoned";
      reject(error);
    };
    const request = factory.open(databaseName, 4);
    request.onupgradeneeded = () => {
      const database = request.result;
      const transaction = request.transaction;
      if (transaction === null) {
        request.transaction?.abort();
        return;
      }
      if (!database.objectStoreNames.contains("records")) {
        database.createObjectStore("records", { keyPath: "id", autoIncrement: true });
      }
      if (!database.objectStoreNames.contains("recordMetadata")) {
        const metadata = database.createObjectStore("recordMetadata", { keyPath: "id" });
        const records = transaction.objectStore("records");
        const cursorRequest = records.openCursor();
        cursorRequest.onsuccess = () => {
          const cursor = cursorRequest.result;
          if (cursor === null) return;
          const record = cursor.value as { readonly savedAt?: unknown };
          if (typeof record.savedAt === "string") {
            metadata.put({ id: Number(cursor.primaryKey), savedAt: record.savedAt });
          }
          cursor.continue();
        };
      }
      if (!database.objectStoreNames.contains("personalBests")) {
        database.createObjectStore("personalBests", { keyPath: "key" });
      }
      if (!database.objectStoreNames.contains("personalBestIndexState")) {
        database.createObjectStore("personalBestIndexState", { keyPath: "key" });
      }
    };
    request.onsuccess = () => {
      if (ownership === "transferred" || ownership === "closed") return;
      const database = request.result;
      if (ownership === "abandoned") {
        ownership = "closed";
        database.close();
        return;
      }
      ownership = "transferred";
      database.onversionchange = () => { database.close(); };
      resolve(database);
    };
    request.onerror = () => { abandon(request.error ?? new Error("IndexedDB database open failed")); };
    request.onblocked = () => { abandon(new Error("IndexedDB database upgrade is blocked")); };
  });
}

function resolvePersonalBestUpdate(id: number, selection: PersonalBestSelectionPort): PersonalBestUpdate {
  if (!selection.is_eligible()) return { kind: "ineligible" };
  const key = selection.key_hex();
  return selection.candidate_is_best()
    ? { kind: "candidate", id, key }
    : { kind: "existing", id: selection.selected_existing_id(), key };
}

function isStoredFlightRecord(value: unknown): value is StoredFlightRecord {
  return typeof value === "object" && value !== null
    && "id" in value && typeof value.id === "number" && Number.isSafeInteger(value.id) && value.id > 0
    && "savedAt" in value && typeof value.savedAt === "string"
    && "json" in value && typeof value.json === "string";
}

interface StoredFlightRecordMetadata {
  readonly id: number;
  readonly savedAt: string;
}

function isStoredFlightRecordMetadata(value: unknown): value is StoredFlightRecordMetadata {
  return typeof value === "object" && value !== null
    && "id" in value && typeof value.id === "number" && Number.isSafeInteger(value.id) && value.id > 0
    && "savedAt" in value && typeof value.savedAt === "string";
}

function isPersonalBestEntry(value: unknown): value is { readonly key: string; readonly recordId: number } {
  return typeof value === "object" && value !== null
    && "key" in value && typeof value.key === "string" && /^[0-9a-f]{64}$/.test(value.key)
    && "recordId" in value && typeof value.recordId === "number"
    && Number.isSafeInteger(value.recordId) && value.recordId > 0;
}

function isPersonalBestIndexState(value: unknown): value is { readonly key: typeof personalBestIndexRevision } {
  return typeof value === "object" && value !== null && "key" in value && value.key === personalBestIndexRevision;
}

function validateFinalizedRecord(json: string): void {
  if (new TextEncoder().encode(json).byteLength > maximumRecordBytes) {
    throw new RangeError("Flight record exceeds the persistence size limit");
  }
  let value: unknown;
  try {
    value = JSON.parse(json) as unknown;
  } catch {
    throw new TypeError("Rust returned invalid flight record JSON");
  }
  if (!isObject(value)) throw new TypeError("Flight record document must be an object");
  const document = value as { readonly header?: unknown; readonly samples?: unknown; readonly finalization?: unknown };
  if (!isObject(document.header)
      || !Array.isArray(document.samples)
      || document.samples.length === 0
      || !isObject(document.finalization)) {
    throw new TypeError("Only a finalized flight record can be persisted");
  }
}

function isObject(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}
