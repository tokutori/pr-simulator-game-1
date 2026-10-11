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
  add(json: string, savedAt: string, selection: PersonalBestSelectionPort): Promise<number>;
  get(id: number): Promise<StoredFlightRecord | null>;
  getAll(): Promise<readonly StoredFlightRecordSummary[]>;
}

const maximumRecordBytes = 16 * 1024 * 1024;

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
      const id = await this.persistence.add(json, savedAt, selection);
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
    const records = await this.persistence.getAll();
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
    json: string, savedAt: string, selection: PersonalBestSelectionPort
  ): Promise<number> {
    return this.withDatabase((database) => new Promise<number>((resolve, reject) => {
      const transaction = database.transaction(["records", "recordMetadata", "personalBests"], "readwrite");
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
                if (recordSchemaVersion(value.json) === 6) selection.consider_existing(Number(cursor.primaryKey), value.json);
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
                if (recordSchemaVersion(existing.json) === 6) selection.consider_existing(indexed.recordId, existing.json);
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
      addWithPersonalBest();
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

  async getAll(): Promise<readonly StoredFlightRecordSummary[]> {
    return this.withDatabase((database) => new Promise<readonly StoredFlightRecordSummary[]>((resolve, reject) => {
      const transaction = database.transaction(["recordMetadata", "personalBests"], "readonly");
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
        readPersonalBestIndex();
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

function openDatabase(factory: IDBFactory, databaseName: string): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    let ownership: "pending" | "abandoned" | "transferred" | "closed" = "pending";
    const abandon = (error: Error): void => {
      if (ownership !== "pending") return;
      ownership = "abandoned";
      reject(error);
    };
    const request = factory.open(databaseName, 4);
    request.onupgradeneeded = (event) => {
      const database = request.result;
      const transaction = request.transaction;
      if (event.oldVersion !== 0) {
        abandon(new Error("Unsupported flight record database version; stored data is retained"));
        transaction?.abort();
        return;
      }
      if (transaction === null) {
        request.transaction?.abort();
        return;
      }
      if (!database.objectStoreNames.contains("records")) {
        database.createObjectStore("records", { keyPath: "id", autoIncrement: true });
      }
      if (!database.objectStoreNames.contains("recordMetadata")) {
        database.createObjectStore("recordMetadata", { keyPath: "id" });
      }
      if (!database.objectStoreNames.contains("personalBests")) {
        database.createObjectStore("personalBests", { keyPath: "key" });
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
  if (value.schema_version !== 6) throw new RangeError("Unsupported flight record schema version");
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

function recordSchemaVersion(json: string): number {
  const value: unknown = JSON.parse(json);
  if (!isObject(value) || typeof value.schema_version !== "number" || !Number.isSafeInteger(value.schema_version)) {
    throw new TypeError("Stored flight record must declare a schema version");
  }
  return value.schema_version;
}
