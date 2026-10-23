import { Window } from "happy-dom";
import { describe, expect, it, vi } from "vitest";
import { BrowserFlightLogDownload } from "../../web/src/app/browser-flight-log-download.js";
import type { ApplicationRequest } from "../../web/src/generated/application-contracts.js";

const request: Extract<ApplicationRequest, { kind: "download_file" }> = {
  kind: "download_file", request_id: 1, bytes: [0, 10, 128, 255], file_name: "flight.json", mime_type: "application/json"
};

describe("Browser file download I/O", () => {
  it("submits exact owner bytes and releases actual URLs after the browser activation", async () => {
    const window = new Window();
    const documentRef = window.document as unknown as Document;
    const blobs: Blob[] = [];
    const timers: (() => void)[] = [];
    const revoke = vi.fn();
    const cancel = vi.fn();
    const download = new BrowserFlightLogDownload(documentRef, { createObjectURL(value) { blobs.push(value); return "blob:flight"; }, revokeObjectURL: revoke },
      { schedule(callback, delay) { expect(delay).toBe(60_000); timers.push(callback); return cancel; } });
    const click = vi.spyOn(window.HTMLAnchorElement.prototype, "click").mockImplementation(function (this: HTMLAnchorElement) {
      expect(this.isConnected).toBe(true);
      expect(this.download).toBe(request.file_name);
      expect(this.href).toBe("blob:flight");
      expect(revoke).not.toHaveBeenCalled();
    });
    try {
      download.download(request);
      expect(click).toHaveBeenCalledOnce();
      expect(documentRef.querySelector("a")).toBeNull();
      const blob = blobs[0];
      const expire = timers[0];
      if (blob === undefined || expire === undefined) throw new Error("Browser I/O handles were not created");
      expect(blob.type).toBe(request.mime_type);
      expect([...new Uint8Array(await blob.arrayBuffer())]).toEqual(request.bytes);
      expire();
      download.dispose();
      expect(revoke).toHaveBeenCalledExactlyOnceWith("blob:flight");
      expect(() => { download.download(request); }).toThrow("disposed");
    } finally { click.mockRestore(); download.dispose(); await window.happyDOM.abort(); }
  });

  it("detaches handles before pagehide re-entry and independently releases every URL after cleanup exceptions", async () => {
    const window = new Window();
    const revoke = vi.fn();
    const cancel = vi.fn(() => { throw new Error("timer cleanup failed"); });
    let serial = 0;
    const download = new BrowserFlightLogDownload(window.document as unknown as Document,
      { createObjectURL: () => `blob:${String(++serial)}`, revokeObjectURL: revoke }, { schedule: () => cancel });
    const click = vi.spyOn(window.HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
    try {
      download.download(request);
      download.download(request);
      expect(() => { download.dispose(); }).toThrow("Download cleanup failed");
      expect(revoke.mock.calls).toEqual([["blob:1"], ["blob:2"]]);
      expect(cancel).toHaveBeenCalledTimes(2);
      download.dispose();
      expect(revoke).toHaveBeenCalledTimes(2);
      const reentrant = new BrowserFlightLogDownload(window.document as unknown as Document,
        { createObjectURL: () => "blob:reentrant", revokeObjectURL: revoke });
      click.mockImplementation(() => { reentrant.dispose(); });
      reentrant.download(request);
      expect(revoke).toHaveBeenLastCalledWith("blob:reentrant");
      expect(revoke).toHaveBeenCalledTimes(3);
    } finally { click.mockRestore(); download.dispose(); await window.happyDOM.abort(); }
  });
});
