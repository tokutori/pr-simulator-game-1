import { IDBFactory } from "fake-indexeddb";
import { Window } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApplicationRuntimeBridge } from "../../web/pkg/birdman_game_wasm.js";
import type { ApplicationDrawingOutput, ApplicationInput, ApplicationRendererObservation, ApplicationRequest, DisplayRequest } from "../../web/src/generated/application-contracts.js";
import { applicationPlatformFixture } from "./application-fixture.js";
import { BrowserFlightLogDownload } from "../../web/src/app/browser-flight-log-download.js";

type Prepare = Extract<ApplicationRequest, { kind: "prepare_resources" }>;
type Resource = Extract<ApplicationRequest, { kind: "commit_resources" | "cancel_resources" | "release_resources" }>;
interface Driver {
  readonly requests: Prepare[];
  readonly dispose: ReturnType<typeof vi.fn>;
  readonly apply: ReturnType<typeof vi.fn>;
  finish(missing?: boolean): void;
}
const capture = vi.hoisted(() => ({ driver: null as Driver | null, permission: (): void => undefined }));

vi.mock("../../web/src/render/engines/three/application-renderer.js", () => ({
  ThreeApplicationRenderer: class {
    readonly requests: Prepare[] = [];
    readonly apply = vi.fn<(drawing: ApplicationDrawingOutput) => void>();
    readonly dispose = vi.fn();
    private readonly candidates = new Map<number, Prepare>();
    private readonly pending = new Map<number, { request: Prepare; cancelled: boolean }>();
    constructor(_canvas: HTMLCanvasElement, private readonly id: number, _bytes: (id: number) => Uint8Array,
      private readonly notify: (input: ApplicationInput) => void) { capture.driver = this; }
    observation(): ApplicationRendererObservation {
      return { resource_id: this.id, initial_quality: { scale: "100", shadow: "full", water_quality: "high" },
        extent_px: [1280, 720], screen_extent_px: [1280, 720], xr_extent_px: null,
        gpu_timing: { kind: "unsupported", cause: "physical timer is unavailable" } };
    }
    prepare(request: Prepare): void {
      this.requests.push(request);
      this.pending.set(request.request_id, { request, cancelled: false });
    }
    finish(missing = false): void {
      for (const [id, preparation] of this.pending) {
        this.pending.delete(id);
        if (!preparation.cancelled && !missing) this.candidates.set(id, preparation.request);
        this.notify({ kind: "resources", request_id: id, observation: preparation.cancelled
          ? { kind: "cancelled" } : { kind: "prepared", resource_id: id } });
      }
    }
    owns(request: Resource): boolean {
      return request.kind === "cancel_resources" ? this.pending.has(request.request_id) : this.candidates.has(request.resource_id);
    }
    ownsDisplay(): boolean { return false; }
    execute(request: Resource): void {
      if (request.kind === "cancel_resources") {
        const preparation = this.pending.get(request.request_id);
        if (preparation !== undefined) preparation.cancelled = true;
      } else if (request.kind === "release_resources") {
        this.candidates.delete(request.resource_id);
        this.notify({ kind: "resources_released", resource_id: request.resource_id, failure: null });
      } else {
        const candidate = this.candidates.get(request.resource_id);
        if (candidate === undefined) throw new Error("Missing physical GPU candidate");
        this.notify({ kind: "resources", request_id: request.request_id, observation: { kind: "committed", resource_id: request.resource_id,
          actual: { applied: candidate.graphics.quality, extent_px: candidate.graphics.extent_px } } });
      }
    }
    display(request: DisplayRequest): void {
      if (request.kind !== "permission") throw new Error("Unexpected display I/O in this fixture");
      capture.permission();
      this.notify({ kind: "display_completed", request_id: request.request_id, completion: { kind: "failed", cause: "Permission denied by platform" } });
    }
  }
}));

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); capture.driver = null; });

async function browserFixture() {
  const browser = new Window({ url: "http://localhost/" });
  browser.document.documentElement.lang = "ja";
  browser.document.documentElement.style.font = "400 16px Arial";
  Object.defineProperty(browser.document, "fonts", { value: Object.assign(new browser.EventTarget(), { ready: Promise.resolve() }) });
  Object.defineProperty(browser, "indexedDB", { value: new IDBFactory() });
  for (const [key, value] of Object.entries({ window: browser, document: browser.document, navigator: browser.navigator,
    HTMLElement: browser.HTMLElement, HTMLInputElement: browser.HTMLInputElement, HTMLSelectElement: browser.HTMLSelectElement,
    MutationObserver: browser.MutationObserver, getComputedStyle: browser.getComputedStyle.bind(browser) })) vi.stubGlobal(key, value);
  vi.spyOn(browser.HTMLCanvasElement.prototype, "getContext").mockImplementation(function (this: HTMLCanvasElement) {
    const context = { canvas: this, font: "16px Arial", measureText(value: string) {
      const size = Number.parseFloat(context.font.match(/([\d.]+)px/)?.[1] ?? "16");
      const width = Array.from(value).length * size * 0.7;
      return { width, actualBoundingBoxLeft: 0, actualBoundingBoxRight: width, actualBoundingBoxAscent: size * 0.8, actualBoundingBoxDescent: size * 0.2 };
    } };
    return context as unknown as ReturnType<typeof browser.HTMLCanvasElement.prototype.getContext>;
  });
  const { BrowserApplication } = await import("../../web/src/browser-application.js");
  const mount = browser.document.createElement("main") as unknown as HTMLElement;
  browser.document.body.append(mount as unknown as import("happy-dom").HTMLElement);
  const free = vi.spyOn(ApplicationRuntimeBridge.prototype, "free");
  const update = vi.spyOn(ApplicationRuntimeBridge.prototype, "update");
  new BrowserApplication(mount, applicationPlatformFixture().capabilities);
  const driver = capture.driver;
  if (driver === null) throw new Error("Main did not execute CreateRenderer");
  const page = (type: "pagehide" | "pageshow", persisted: boolean) => {
    const event = new browser.PageTransitionEvent(type);
    Object.defineProperty(event, "persisted", { value: persisted });
    browser.dispatchEvent(event);
  };
  return { browser, mount, free, update, driver, page };
}

describe("Single browser runtime ownership", () => {
  it("retains an in-flight preparation through BFCache and executes permission inside the originating click", async () => {
    const fixture = await browserFixture();
    try {
      expect(fixture.driver.requests).toHaveLength(1);
      expect(fixture.driver.requests[0]?.graphics.quality).toEqual({ scale: "50", shadow: "quarter", water_quality: "low" });
      fixture.page("pagehide", true);
      fixture.driver.finish();
      expect(fixture.free).not.toHaveBeenCalled();
      expect(fixture.driver.dispose).not.toHaveBeenCalled();
      fixture.page("pageshow", true);
      fixture.driver.finish();
      const hmd = fixture.mount.querySelector<HTMLButtonElement>('[data-control-id="boot-enter-hmd"]');
      if (hmd === null) throw new Error("Retained Title is not interactive");
      let inClick = false;
      const permission = vi.fn(() => { expect(inClick).toBe(true); });
      capture.permission = permission;
      inClick = true;
      hmd.click();
      inClick = false;
      expect(permission).toHaveBeenCalledOnce();
      const start = fixture.mount.querySelector<HTMLButtonElement>('[data-control-id="game-title-start"]');
      if (start === null) throw new Error("Permission failure did not restore Title controls");
      start.click();
      expect(fixture.mount.querySelector('[data-control-id="game-setup-start"]')).not.toBeNull();
      fixture.page("pagehide", false);
      fixture.driver.finish();
      expect(fixture.driver.dispose).toHaveBeenCalledOnce();
      expect(fixture.free).toHaveBeenCalledOnce();
      const updates = fixture.update.mock.calls.length;
      fixture.browser.document.documentElement.style.fontSize = "20px";
      fixture.page("pageshow", true);
      await Promise.resolve();
      expect(fixture.update).toHaveBeenCalledTimes(updates);
    } finally { fixture.page("pagehide", false); fixture.driver.finish(); await fixture.browser.happyDOM.abort(); }
  });

  it.each(["cancelled preparation", "missing prepared handle"])("frees once after the actual %s terminal and every cleanup", async failure => {
    const fixture = await browserFixture();
    const cleanup = vi.spyOn(BrowserFlightLogDownload.prototype, "dispose").mockImplementation(() => { throw new Error("download cleanup failed"); });
    const diagnostic = vi.spyOn(console, "error").mockImplementation(() => undefined);
    try {
      if (failure === "missing prepared handle") {
        fixture.driver.finish(true);
        expect(fixture.update.mock.calls.some(([json]) => json.includes('"binding_lost"'))).toBe(true);
      }
      fixture.page("pagehide", false);
      if (failure === "cancelled preparation") expect(fixture.free).not.toHaveBeenCalled();
      fixture.driver.finish();
      expect(fixture.driver.dispose).toHaveBeenCalledOnce();
      expect(cleanup).toHaveBeenCalledOnce();
      expect(fixture.free).toHaveBeenCalledOnce();
      expect(diagnostic).toHaveBeenCalledWith(expect.any(AggregateError));
      fixture.page("pagehide", false);
      expect(fixture.free).toHaveBeenCalledOnce();
    } finally { await fixture.browser.happyDOM.abort(); }
  });
});
