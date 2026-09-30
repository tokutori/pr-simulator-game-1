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

export interface SavedFlightRecord extends StoredFlightRecord {
  readonly personalBest: PersonalBestUpdate;
}

export interface FlightRecordPersistencePort {
  add(json: string, savedAt: string, selection?: PersonalBestSelectionPort): Promise<number>;
  get(id: number): Promise<StoredFlightRecord | null>;
  getAll(createSelection?: (json: string) => PersonalBestSelectionPort): Promise<readonly StoredFlightRecordSummary[]>;
}

const maximumRecordBytes = 16 * 1024 * 1024;

export class FlightRecordRepository {
  constructor(
    private readonly persistence: FlightRecordPersistencePort,
    private readonly now: () => Date = () => new Date(),
    private readonly createSelection?: (json: string) => PersonalBestSelectionPort
  ) {}

  async saveFrom(session: FlightRecordExportPort): Promise<SavedFlightRecord> {
    const json = session.export_flight_record_json();
    validateFinalizedRecord(json);
    const savedAt = this.now().toISOString();
    const selection = this.createSelection?.(json);
    try {
      const id = await this.persistence.add(json, savedAt, selection);
      return Object.freeze({ id, savedAt, json, personalBest: resolvePersonalBestUpdate(id, selection) });
    } finally {
      selection?.free();
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

  async add(json: string, savedAt: string, selection?: PersonalBestSelectionPort): Promise<number> {
    return this.withDatabase((database) => new Promise<number>((resolve, reject) => {
      const stores = selection?.is_eligible()
        ? ["records", "recordMetadata", "personalBests"]
        : ["records", "recordMetadata"];
      const transaction = database.transaction(stores, "readwrite");
      const records = transaction.objectStore("records");
      let id: number | null = null;
      let failure: unknown;
      const addCandidate = (): void => {
        const request = records.add({ savedAt, json });
        request.onsuccess = () => {
          id = Number(request.result);
          transaction.objectStore("recordMetadata").add({ id, savedAt });
          if (selection?.is_eligible()) {
            const key = selection.key_hex();
            const selectedId = selection.candidate_is_best() ? id : selection.selected_existing_id();
            if (key.length !== 64 || !Number.isSafeInteger(selectedId) || selectedId < 1) {
              failure = new TypeError("Rust returned an invalid Personal Best selection");
              transaction.abort();
              return;
            }
            transaction.objectStore("personalBests").put({ key, recordId: selectedId });
          }
        };
        request.onerror = () => { reject(request.error ?? new Error("IndexedDB record insertion failed")); };
      };
      if (selection?.is_eligible()) {
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
              failure = new TypeError("IndexedDB flight record is malformed");
              transaction.abort();
              return;
            }
            try {
              selection.consider_existing(Number(cursor.primaryKey), value.json);
            } catch (error: unknown) {
              failure = error;
              transaction.abort();
              return;
            }
            cursor.continue();
          };
          cursorRequest.onerror = () => { reject(cursorRequest.error ?? new Error("IndexedDB record scan failed")); };
        };
        const indexedRequest = transaction.objectStore("personalBests").get(key);
        indexedRequest.onsuccess = () => {
          const indexed: unknown = indexedRequest.result;
          if (indexed === undefined) {
            scanExistingRecords();
            return;
          }
          if (!isPersonalBestEntry(indexed) || indexed.key !== key) {
            failure = new TypeError("IndexedDB Personal Best index is malformed");
            transaction.abort();
            return;
          }
          const existingRequest = records.get(indexed.recordId);
          existingRequest.onsuccess = () => {
            const existing: unknown = existingRequest.result;
            if (!isStoredFlightRecord(existing)) {
              failure = new TypeError("IndexedDB Personal Best record is missing or malformed");
              transaction.abort();
              return;
            }
            try {
              selection.consider_existing(indexed.recordId, existing.json);
            } catch (error: unknown) {
              failure = error;
              transaction.abort();
              return;
            }
            addCandidate();
          };
          existingRequest.onerror = () => {
            reject(existingRequest.error ?? new Error("IndexedDB Personal Best record read failed"));
          };
        };
        indexedRequest.onerror = () => {
          reject(indexedRequest.error ?? new Error("IndexedDB Personal Best index read failed"));
        };
      } else {
        addCandidate();
      }
      transaction.oncomplete = () => {
        if (id === null) reject(new Error("IndexedDB completed without a record key"));
        else resolve(id);
      };
      transaction.onabort = () => {
        const error = failure instanceof Error
          ? failure
          : transaction.error ?? new Error("IndexedDB record insertion was aborted");
        reject(error);
      };
      transaction.onerror = () => { reject(transaction.error ?? new Error("IndexedDB record insertion failed")); };
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
    createSelection?: (json: string) => PersonalBestSelectionPort
  ): Promise<readonly StoredFlightRecordSummary[]> {
    return this.withDatabase((database) => new Promise<readonly StoredFlightRecordSummary[]>((resolve, reject) => {
      const transaction = createSelection === undefined
        ? database.transaction(["recordMetadata", "personalBests"], "readonly")
        : database.transaction(["records", "recordMetadata", "personalBests", "personalBestIndexState"], "readwrite");
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
        personalBestRequest.onerror = () => {
          abort(personalBestRequest.error ?? new Error("IndexedDB Personal Best list failed"));
        };
      };
      const resolvePersonalBestIndex = (): void => {
        if (createSelection === undefined) {
          readPersonalBestIndex();
          return;
        }
        const stateStore = transaction.objectStore("personalBestIndexState");
        const stateRequest = stateStore.get("canonical-v1");
        stateRequest.onsuccess = () => {
          const state: unknown = stateRequest.result;
          if (state === undefined) {
            rebuildPersonalBestIndex(
              transaction,
              personalBests,
              stateStore,
              createSelection,
              (recordIds) => { personalBestRecordIds = recordIds; },
              abort
            );
            return;
          }
          if (!isPersonalBestIndexState(state)) {
            abort(new TypeError("IndexedDB Personal Best index state is malformed"));
            return;
          }
          readPersonalBestIndex();
        };
        stateRequest.onerror = () => {
          abort(stateRequest.error ?? new Error("IndexedDB Personal Best index state read failed"));
        };
      };
      metadataRequest.onsuccess = () => {
        const result: unknown = metadataRequest.result;
        if (!Array.isArray(result) || !result.every(isStoredFlightRecordMetadata)) {
          abort(new TypeError("IndexedDB record metadata is malformed"));
          return;
        }
        metadata = result;
        resolvePersonalBestIndex();
      };
      metadataRequest.onerror = () => { abort(metadataRequest.error ?? new Error("IndexedDB record list failed")); };
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
      transaction.onerror = () => { reject(transaction.error ?? new Error("IndexedDB record list failed")); };
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

function rebuildPersonalBestIndex(
  transaction: IDBTransaction,
  personalBests: IDBObjectStore,
  stateStore: IDBObjectStore,
  createSelection: (json: string) => PersonalBestSelectionPort,
  completed: (recordIds: ReadonlySet<number>) => void,
  abort: (error: unknown) => void
): void {
  const records = transaction.objectStore("records");
  const selections = new Map<string, { readonly candidateId: number; readonly selection: PersonalBestSelectionPort }>();
  let activeSelection: PersonalBestSelectionPort | null = null;
  const releaseSelections = (): void => {
    activeSelection?.free();
    activeSelection = null;
    for (const { selection } of selections.values()) selection.free();
    selections.clear();
  };
  const fail = (error: unknown): void => {
    releaseSelections();
    abort(error);
  };
  const clearRequest = personalBests.clear();
  clearRequest.onerror = () => { fail(clearRequest.error ?? new Error("IndexedDB Personal Best index reset failed")); };
  clearRequest.onsuccess = () => {
    const cursorRequest = records.openCursor();
    cursorRequest.onerror = () => { fail(cursorRequest.error ?? new Error("IndexedDB Personal Best migration scan failed")); };
    cursorRequest.onsuccess = () => {
      const cursor = cursorRequest.result;
      if (cursor === null) {
        const recordIds = new Set<number>();
        const selected: { readonly key: string; readonly recordId: number }[] = [];
        try {
          for (const [key, entry] of selections) {
            const recordId = entry.selection.candidate_is_best()
              ? entry.candidateId
              : entry.selection.selected_existing_id();
            if (key.length !== 64 || !/^[0-9a-f]{64}$/.test(key) || !Number.isSafeInteger(recordId) || recordId < 1) {
              fail(new TypeError("Rust returned an invalid Personal Best migration result"));
              return;
            }
            selected.push({ key, recordId });
          }
          for (const entry of selected) personalBests.put(entry);
          stateStore.put({ key: "canonical-v1" });
        } catch (error: unknown) {
          fail(error);
          return;
        }
        for (const entry of selections.values()) entry.selection.free();
        selections.clear();
        for (const { recordId } of selected) {
          recordIds.add(recordId);
        }
        completed(recordIds);
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
          selections.set(key, { candidateId: id, selection: activeSelection });
          activeSelection = null;
        } else {
          existing.selection.consider_existing(id, value.json);
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
      const database = request.result;
      database.onversionchange = () => { database.close(); };
      resolve(database);
    };
    request.onerror = () => { reject(request.error ?? new Error("IndexedDB database open failed")); };
    request.onblocked = () => { reject(new Error("IndexedDB database upgrade is blocked")); };
  });
}

function resolvePersonalBestUpdate(id: number, selection: PersonalBestSelectionPort | undefined): PersonalBestUpdate {
  if (selection === undefined) return { kind: "not-evaluated" };
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

function isPersonalBestIndexState(value: unknown): value is { readonly key: "canonical-v1" } {
  return typeof value === "object" && value !== null && "key" in value && value.key === "canonical-v1";
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
