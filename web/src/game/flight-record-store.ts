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
}

export interface FlightRecordPersistencePort {
  add(json: string, savedAt: string): Promise<number>;
  get(id: number): Promise<StoredFlightRecord | null>;
  getAll(): Promise<readonly StoredFlightRecordSummary[]>;
}

const maximumRecordBytes = 16 * 1024 * 1024;

export class FlightRecordRepository {
  constructor(
    private readonly persistence: FlightRecordPersistencePort,
    private readonly now: () => Date = () => new Date()
  ) {}

  async saveFrom(session: FlightRecordExportPort): Promise<StoredFlightRecord> {
    const json = session.export_flight_record_json();
    validateFinalizedRecord(json);
    const savedAt = this.now().toISOString();
    const id = await this.persistence.add(json, savedAt);
    return Object.freeze({ id, savedAt, json });
  }

  async load(id: number): Promise<string | null> {
    if (!Number.isSafeInteger(id) || id < 1) throw new RangeError("Record identifier must be a positive integer");
    return (await this.persistence.get(id))?.json ?? null;
  }

  async list(): Promise<readonly StoredFlightRecordSummary[]> {
    const records = await this.persistence.getAll();
    return Object.freeze([...records]
      .sort((left, right) => right.id - left.id)
      .map(({ id, savedAt }) => Object.freeze({ id, savedAt })));
  }
}

export class IndexedDbFlightRecordPersistence implements FlightRecordPersistencePort {
  constructor(
    private readonly factory: IDBFactory,
    private readonly databaseName = "birdman-flight-records"
  ) {}

  async add(json: string, savedAt: string): Promise<number> {
    return this.withDatabase((database) => new Promise<number>((resolve, reject) => {
      const transaction = database.transaction(["records", "recordMetadata"], "readwrite");
      const request = transaction.objectStore("records").add({ savedAt, json });
      let id: number | null = null;
      request.onsuccess = () => {
        id = Number(request.result);
        transaction.objectStore("recordMetadata").add({ id, savedAt });
      };
      request.onerror = () => { reject(request.error ?? new Error("IndexedDB record insertion failed")); };
      transaction.oncomplete = () => {
        if (id === null) reject(new Error("IndexedDB completed without a record key"));
        else resolve(id);
      };
      transaction.onabort = () => { reject(transaction.error ?? new Error("IndexedDB record insertion was aborted")); };
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

  async getAll(): Promise<readonly StoredFlightRecordSummary[]> {
    return this.withDatabase((database) => new Promise<readonly StoredFlightRecordSummary[]>((resolve, reject) => {
      const transaction = database.transaction("recordMetadata", "readonly");
      const request = transaction.objectStore("recordMetadata").getAll();
      let records: readonly StoredFlightRecordSummary[] | null = null;
      request.onsuccess = () => {
        const result: unknown = request.result;
        if (Array.isArray(result) && result.every(isStoredFlightRecordSummary)) {
          records = Object.freeze(result);
        }
      };
      request.onerror = () => { reject(request.error ?? new Error("IndexedDB record list failed")); };
      transaction.oncomplete = () => {
        if (records === null) reject(new TypeError("IndexedDB record metadata is malformed"));
        else resolve(records);
      };
      transaction.onabort = () => { reject(transaction.error ?? new Error("IndexedDB record list was aborted")); };
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

function openDatabase(factory: IDBFactory, databaseName: string): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const request = factory.open(databaseName, 2);
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

function isStoredFlightRecord(value: unknown): value is StoredFlightRecord {
  return typeof value === "object" && value !== null
    && "id" in value && typeof value.id === "number" && Number.isSafeInteger(value.id) && value.id > 0
    && "savedAt" in value && typeof value.savedAt === "string"
    && "json" in value && typeof value.json === "string";
}

function isStoredFlightRecordSummary(value: unknown): value is StoredFlightRecordSummary {
  return typeof value === "object" && value !== null
    && "id" in value && typeof value.id === "number" && Number.isSafeInteger(value.id) && value.id > 0
    && "savedAt" in value && typeof value.savedAt === "string";
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
