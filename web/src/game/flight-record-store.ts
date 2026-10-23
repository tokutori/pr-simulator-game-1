import type { ApplicationInput, ApplicationRequest, RecordPreviewRow, RecordWriteObservation } from "../generated/application-contracts.js";

type StorageRequest = Extract<ApplicationRequest, {
  kind: "begin_record_preview" | "record_preview" | "begin_record_save" | "record_write" | "read_record";
}>;

interface TransactionHandle {
  readonly transaction: IDBTransaction;
  cursor: IDBCursorWithValue | null;
  failure: unknown;
  cancelled: boolean;
}

export class BrowserFlightRecordStore {
  private database: Promise<IDBDatabase> | null = null;
  private readonly transactions = new Map<number, TransactionHandle>();
  private readonly opening = new Map<number, { cancelled: boolean }>();
  private disposed = false;

  constructor(private readonly factory: IDBFactory, private readonly notify: (input: ApplicationInput) => void,
    private readonly databaseName = "birdman-flight-records") {}

  execute(request: StorageRequest): void {
    if (this.disposed) throw new Error("The record database is disposed");
    if (request.kind === "record_write" || request.kind === "record_preview") {
      const handle = this.transactions.get(request.request_id);
      if (handle === undefined) throw new Error("The requested record transaction is not active");
      try {
        if (request.kind === "record_write") this.write(handle, request);
        else this.preview(handle, request);
      } catch (cause) {
        handle.failure = cause;
        handle.transaction.abort();
      }
      return;
    }
    const operation = { cancelled: false };
    this.opening.set(request.request_id, operation);
    void this.open().then(database => {
      this.opening.delete(request.request_id);
      if (operation.cancelled) {
        this.notify({ kind: "record_io_cancelled", request_id: request.request_id, failure: null });
        return;
      }
      if (request.kind === "read_record") this.read(database, request);
      else this.begin(database, request);
    }).catch((cause: unknown) => {
      this.opening.delete(request.request_id);
      const message = failureMessage(cause);
      if (operation.cancelled) {
        this.notify({ kind: "record_io_cancelled", request_id: request.request_id, failure: message });
        return;
      }
      if (request.kind === "read_record") this.notify({ kind: "record_loaded", request_id: request.request_id, completion: { kind: "failed", cause: message } });
      else if (request.kind === "begin_record_preview") this.notify({ kind: "record_preview_observed", request_id: request.request_id, observation: { kind: "failed", cause: message } });
      else this.notify({ kind: "record_save_observed", request_id: request.request_id, observation: { kind: "failed", cause: message } });
    });
  }

  private begin(database: IDBDatabase, request: Extract<StorageRequest, { kind: "begin_record_preview" | "begin_record_save" }>): void {
    const transaction = database.transaction(["records", "recordMetadata", "personalBests"], request.kind === "begin_record_save" ? "readwrite" : "readonly");
    const handle: TransactionHandle = { transaction, cursor: null, failure: undefined, cancelled: false };
    this.transactions.set(request.request_id, handle);
    const observe = (observation: RecordWriteObservation): void => {
      this.notify({ kind: "record_save_observed", request_id: request.request_id, observation });
    };
    transaction.oncomplete = () => {
      this.transactions.delete(request.request_id);
      if (this.finishCancellation(request.request_id, handle)) return;
      if (request.kind === "begin_record_save") observe({ kind: "committed" });
      else this.notify({ kind: "record_preview_observed", request_id: request.request_id, observation: { kind: "committed" } });
    };
    transaction.onabort = () => {
      this.transactions.delete(request.request_id);
      if (this.finishCancellation(request.request_id, handle)) return;
      const cause = failureMessage(handle.failure ?? transaction.error ?? new Error("IndexedDB transaction aborted"));
      if (request.kind === "begin_record_save") observe({ kind: "failed", cause });
      else this.notify({ kind: "record_preview_observed", request_id: request.request_id, observation: { kind: "failed", cause } });
    };
    if (request.kind === "begin_record_save") {
      this.notify({ kind: "record_save_opened", request_id: request.request_id, unix_milliseconds: Date.now() });
      return;
    }
    const rows: RecordPreviewRow[] = [];
    const cursorRequest = transaction.objectStore("recordMetadata").openCursor(null, "prev");
    cursorRequest.onsuccess = () => {
      const cursor = cursorRequest.result;
      if (cursor === null) {
        this.notify({ kind: "record_preview_observed", request_id: request.request_id, observation: { kind: "rows", rows } });
        return;
      }
      const recordRequest = transaction.objectStore("records").get(cursor.primaryKey);
      recordRequest.onsuccess = () => {
        rows.push({ key: cursor.primaryKey, metadata: cursor.value as unknown, record: (recordRequest.result as unknown) ?? null });
        if (rows.length < request.limit) cursor.continue();
        else this.notify({ kind: "record_preview_observed", request_id: request.request_id, observation: { kind: "rows", rows } });
      };
    };
  }

  private write(handle: TransactionHandle, request: Extract<StorageRequest, { kind: "record_write" }>): void {
    const observe = (observation: RecordWriteObservation): void => {
      this.notify({ kind: "record_save_observed", request_id: request.request_id, observation });
    };
    const records = handle.transaction.objectStore("records");
    for (const operation of request.requests) {
      switch (operation.kind) {
        case "read_best": {
          const read = handle.transaction.objectStore("personalBests").get(operation.key);
          read.onsuccess = () => { observe({ kind: "best", value: (read.result as unknown) ?? null }); };
          break;
        }
        case "read_record": {
          const read = records.get(operation.id);
          read.onsuccess = () => { observe({ kind: "record", value: (read.result as unknown) ?? null }); };
          break;
        }
        case "scan_records": {
          const cursor = records.openCursor();
          cursor.onsuccess = () => {
            handle.cursor = cursor.result;
            observe({ kind: "scanned", key: cursor.result?.primaryKey ?? null, value: (cursor.result?.value as unknown) ?? null });
          };
          break;
        }
        case "continue_records":
          if (handle.cursor === null) throw new Error("The requested record cursor is not active");
          handle.cursor.continue();
          break;
        case "insert_record": {
          const insert = records.add({ savedAt: operation.saved_at, json: operation.json });
          insert.onsuccess = () => { observe({ kind: "inserted", key: insert.result }); };
          break;
        }
        case "write_metadata": {
          const write = handle.transaction.objectStore("recordMetadata").put({ id: operation.id, savedAt: operation.saved_at });
          write.onsuccess = () => { observe({ kind: "metadata_written" }); };
          break;
        }
        case "write_best": {
          const write = handle.transaction.objectStore("personalBests").put(operation.entry);
          write.onsuccess = () => { observe({ kind: "best_written" }); };
          break;
        }
        case "finish": handle.transaction.commit(); break;
        case "abort": handle.transaction.abort(); break;
      }
    }
  }

  private preview(handle: TransactionHandle, request: Extract<StorageRequest, { kind: "record_preview" }>): void {
    for (const operation of request.requests) {
      switch (operation.kind) {
        case "read_best": {
          const values: unknown[] = new Array<unknown>(operation.keys.length);
          let remaining = operation.keys.length;
          const complete = (): void => { this.notify({ kind: "record_preview_observed", request_id: request.request_id, observation: { kind: "bests", values } }); };
          if (remaining === 0) complete();
          operation.keys.forEach((key, index) => {
            const read = handle.transaction.objectStore("personalBests").get(key);
            read.onsuccess = () => {
              values[index] = (read.result as unknown) ?? null;
              remaining--;
              if (remaining === 0) complete();
            };
          });
          break;
        }
        case "finish": handle.transaction.commit(); break;
        case "abort": handle.transaction.abort(); break;
      }
    }
  }

  private read(database: IDBDatabase, request: Extract<StorageRequest, { kind: "read_record" }>): void {
    const transaction = database.transaction("records", "readonly");
    const handle: TransactionHandle = { transaction, cursor: null, failure: undefined, cancelled: false };
    this.transactions.set(request.request_id, handle);
    const read = transaction.objectStore("records").get(request.record_id);
    transaction.oncomplete = () => {
      this.transactions.delete(request.request_id);
      if (this.finishCancellation(request.request_id, handle)) return;
      const value: unknown = read.result;
      this.notify({ kind: "record_loaded", request_id: request.request_id, completion: value === undefined ? { kind: "missing" } : { kind: "found", value } });
    };
    transaction.onabort = () => {
      this.transactions.delete(request.request_id);
      if (this.finishCancellation(request.request_id, handle)) return;
      this.notify({ kind: "record_loaded", request_id: request.request_id, completion: { kind: "failed", cause: failureMessage(transaction.error ?? new Error("IndexedDB transaction aborted")) } });
    };
  }

  cancel(requestId: number): void {
    const opening = this.opening.get(requestId);
    if (opening !== undefined) { opening.cancelled = true; return; }
    const handle = this.transactions.get(requestId);
    if (handle === undefined) {
      this.notify({ kind: "record_io_cancelled", request_id: requestId, failure: null });
      return;
    }
    handle.cancelled = true;
    try { handle.transaction.abort(); }
    catch (cause) {
      if (!(cause instanceof DOMException && cause.name === "InvalidStateError")) handle.failure = cause;
    }
  }

  private finishCancellation(requestId: number, handle: TransactionHandle): boolean {
    if (!handle.cancelled) return false;
    const failure = handle.failure ?? handle.transaction.error;
    this.notify({ kind: "record_io_cancelled", request_id: requestId, failure: failure == null ? null : failureMessage(failure) });
    return true;
  }

  private open(): Promise<IDBDatabase> {
    if (this.database !== null) return this.database;
    let abandoned = false;
    let upgradeFailure: Error | undefined;
    const opening = new Promise<IDBDatabase>((resolve, reject) => {
      const request = this.factory.open(this.databaseName, 4);
      request.onupgradeneeded = event => {
        if (event.oldVersion !== 0) {
          upgradeFailure = new Error("Unsupported flight record database version; stored data is retained");
          request.transaction?.abort();
          return;
        }
        request.result.createObjectStore("records", { keyPath: "id", autoIncrement: true });
        request.result.createObjectStore("recordMetadata", { keyPath: "id" });
        request.result.createObjectStore("personalBests", { keyPath: "key" });
      };
      request.onsuccess = () => {
        const database = request.result;
        if (this.disposed || abandoned) { database.close(); reject(new Error("The record database is disposed or its open request failed")); return; }
        database.onversionchange = () => {
          database.close();
          if (this.database === opening) this.database = null;
        };
        resolve(database);
      };
      request.onerror = () => { abandoned = true; reject(upgradeFailure ?? request.error ?? new Error("IndexedDB database open failed")); };
      request.onblocked = () => { console.warn("IndexedDB database upgrade is waiting for another connection to close"); };
    });
    this.database = opening;
    void opening.catch(() => { if (this.database === opening) this.database = null; });
    return opening;
  }

  dispose(): void {
    if (this.disposed) return;
    this.disposed = true;
    const failures: unknown[] = [];
    for (const handle of this.transactions.values()) {
      try { handle.transaction.abort(); }
      catch (cause) {
        if (!(cause instanceof DOMException && cause.name === "InvalidStateError")) failures.push(cause);
      }
    }
    this.transactions.clear();
    void this.database?.then(database => { database.close(); }, () => undefined);
    this.database = null;
    if (failures.length > 0) throw new AggregateError(failures, "Record transaction cleanup failed");
  }
}

function failureMessage(cause: unknown): string {
  return cause instanceof Error ? `${cause.name}: ${cause.message}` : String(cause);
}
