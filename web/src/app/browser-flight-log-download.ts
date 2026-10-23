import type { ApplicationRequest } from "../generated/application-contracts.js";

export interface ObjectUrlPort {
  createObjectURL(blob: Blob): string;
  revokeObjectURL(url: string): void;
}

export interface DownloadSchedulerPort {
  schedule(callback: () => void, delayMilliseconds: number): () => void;
}

const browserDownloadScheduler: DownloadSchedulerPort = {
  schedule(callback, delayMilliseconds) {
    const timer = globalThis.setTimeout(callback, delayMilliseconds);
    return () => { globalThis.clearTimeout(timer); };
  }
};

type DownloadResource =
  | { readonly kind: "active"; readonly urls: Map<string, () => void> }
  | { readonly kind: "disposed" };

export class BrowserFlightLogDownload {
  private resource: DownloadResource = { kind: "active", urls: new Map() };

  constructor(
    private readonly document: Document,
    private readonly urls: ObjectUrlPort = URL,
    private readonly scheduler: DownloadSchedulerPort = browserDownloadScheduler
  ) {}

  download(log: Extract<ApplicationRequest, { kind: "download_file" }>): void {
    if (this.resource.kind === "disposed") throw new Error("Flight log download resource is disposed");
    if (this.resource.urls.size >= 8) throw new Error("Object URL capacity exhausted");
    const url = this.urls.createObjectURL(new Blob([Uint8Array.from(log.bytes)], { type: log.mime_type }));
    const owner = this.resource;
    owner.urls.set(url, () => undefined);
    try {
      const anchor = this.document.createElement("a");
      try {
        anchor.href = url;
        anchor.download = log.file_name;
        anchor.hidden = true;
        this.document.body.append(anchor);
        anchor.click();
      } finally {
        anchor.remove();
      }
      if (owner.urls.has(url)) {
        const cancel = this.scheduler.schedule(() => {
          if (owner.urls.delete(url)) this.urls.revokeObjectURL(url);
        }, 60_000);
        if (owner.urls.has(url)) owner.urls.set(url, cancel);
        else cancel();
      }
    } catch (error: unknown) {
      if (owner.urls.delete(url)) {
        this.urls.revokeObjectURL(url);
      }
      throw error;
    }
  }

  dispose(): void {
    const previous = this.resource;
    this.resource = { kind: "disposed" };
    if (previous.kind === "active") {
      const resources = [...previous.urls];
      previous.urls.clear();
      const failures: unknown[] = [];
      for (const [url, cancel] of resources) {
        try { cancel(); } catch (cause) { failures.push(cause); }
        try { this.urls.revokeObjectURL(url); } catch (cause) { failures.push(cause); }
      }
      if (failures.length > 0) throw new AggregateError(failures, "Download cleanup failed");
    }
  }
}
