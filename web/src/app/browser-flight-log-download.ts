import type { FlightLogDownload, FlightLogDownloadPort } from "../game/flight-log-export.js";

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

export class BrowserFlightLogDownload implements FlightLogDownloadPort {
  private resource: DownloadResource = { kind: "active", urls: new Map() };

  constructor(
    private readonly document: Document,
    private readonly urls: ObjectUrlPort = URL,
    private readonly scheduler: DownloadSchedulerPort = browserDownloadScheduler
  ) {}

  download(log: FlightLogDownload): void {
    if (this.resource.kind === "disposed") throw new Error("Flight log download resource is disposed");
    if (this.resource.urls.size >= 8) throw new Error("飛行ログのdownload要求が上限8件に達した。60秒後に再操作する。");
    const type = log.format === "csv" ? "text/csv;charset=utf-8" : "application/json;charset=utf-8";
    const url = this.urls.createObjectURL(new Blob([log.text], { type }));
    const owner = this.resource;
    owner.urls.set(url, () => undefined);
    try {
      const anchor = this.document.createElement("a");
      try {
        anchor.href = url;
        anchor.download = sanitizeFlightLogFilename(log.filename, log.format);
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
      for (const [url, cancel] of previous.urls) {
        cancel();
        this.urls.revokeObjectURL(url);
      }
      previous.urls.clear();
    }
  }
}

export function sanitizeFlightLogFilename(filename: string, format: FlightLogDownload["format"]): string {
  const stem = filename.replace(/\.[^.]*$/, "").replace(/[^a-zA-Z0-9_-]/g, "-").slice(0, 80);
  return `${stem || "flight-log"}.${format}`;
}
