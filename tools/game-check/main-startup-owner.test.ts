import { IDBFactory } from "fake-indexeddb";
import { Window } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ApplicationRuntimeBridge } from "../../web/pkg/birdman_game_wasm.js";
import type { ApplicationDrawingOutput, ApplicationInput, ApplicationObservation, ApplicationRendererObservation, ApplicationRequest, DisplayRequest } from "../../web/src/generated/application-contracts.js";
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
const capture = vi.hoisted(() => ({ driver: null as Driver | null, permission: (): void => undefined, observation: (): void => undefined }));

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
      capture.observation();
      return { resource_id: this.id, initial_quality: { scale: "100", shadow: "full", water_quality: "high" },
        extent_px: [1280, 720], screen_extent_px: [window.innerWidth, window.innerHeight], xr_extent_px: null,
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

afterEach(() => { vi.restoreAllMocks(); vi.unstubAllGlobals(); capture.driver = null; capture.observation = (): void => undefined; });

async function browserFixture(phoneSensors: "probe" | "permission" | null = null) {
  const browser = new Window({ url: "http://localhost/" });
  browser.innerWidth = 1280;
  browser.innerHeight = 720;
  browser.document.documentElement.lang = "ja";
  browser.document.documentElement.style.font = "400 16px Arial";
  Object.defineProperty(browser.document, "fonts", { value: Object.assign(new browser.EventTarget(), { ready: Promise.resolve() }) });
  Object.defineProperty(browser, "indexedDB", { value: new IDBFactory() });
  Object.defineProperty(browser.screen, "orientation", { value: Object.assign(new browser.EventTarget(), { angle: 0 }) });
  for (const [key, value] of Object.entries({ window: browser, document: browser.document, navigator: browser.navigator,
    HTMLElement: browser.HTMLElement, HTMLInputElement: browser.HTMLInputElement, HTMLSelectElement: browser.HTMLSelectElement,
    MutationObserver: browser.MutationObserver, screen: browser.screen, getComputedStyle: browser.getComputedStyle.bind(browser) })) vi.stubGlobal(key, value);
  Object.defineProperty(browser, "DeviceMotionEvent", { value: undefined });
  const listeners = vi.spyOn(browser, "addEventListener");
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
  new BrowserApplication(mount, { ...applicationPlatformFixture().capabilities,
    phone_sensors: phoneSensors === null ? null : { permission_required: phoneSensors === "permission", screen_angle_degrees: 0 } });
  const driver = capture.driver;
  if (driver === null) throw new Error("Main did not execute CreateRenderer");
  const page = (type: "pagehide" | "pageshow", persisted: boolean) => {
    const event = new browser.PageTransitionEvent(type);
    Object.defineProperty(event, "persisted", { value: persisted });
    browser.dispatchEvent(event);
  };
  return { browser, mount, free, update, driver, page, listeners };
}

describe("Single browser runtime ownership", () => {
  it("forwards absolute sensor observations without Motion and retires the actual subscription", async () => {
    const fixture = await browserFixture("probe");
    const inputs = () => fixture.update.mock.calls.map(([json]) => (JSON.parse(json) as ApplicationObservation).input);
    const orientation = (absolute: boolean, alpha: number | null): void => {
      const event = new fixture.browser.Event("deviceorientation");
      Object.assign(event, { alpha, beta: 0, gamma: 0, absolute });
      fixture.browser.dispatchEvent(event);
    };
    try {
      fixture.driver.finish();
      const started = inputs().find(input => input.kind === "phone_sensors_started");
      expect(started).toMatchObject({ failure: null });
      if (started?.kind !== "phone_sensors_started") throw new Error("Phone sensor subscription was not requested");
      expect(fixture.listeners.mock.calls.some(([type]) => type === "devicemotion")).toBe(false);
      const choice = () => fixture.mount.querySelector<HTMLButtonElement>('[data-control-id="boot-enter-phone-vr"]');
      expect(choice()?.disabled).toBe(true);
      orientation(false, 12);
      orientation(true, null);
      expect(choice()?.disabled).toBe(true);
      orientation(true, 12);
      expect(inputs().filter(input => input.kind === "phone_orientation").map(input => ({ resource_id: input.resource_id,
        absolute: input.observation.absolute, alpha: input.observation.alpha }))).toEqual([
        { resource_id: started.resource_id, absolute: false, alpha: 12 },
        { resource_id: started.resource_id, absolute: true, alpha: null },
        { resource_id: started.resource_id, absolute: true, alpha: 12 }
      ]);
      expect(choice()?.disabled).toBe(false);
      expect(inputs()).toContainEqual({ kind: "phone_sensors_stopped", resource_id: started.resource_id, failure: null });
      const updates = fixture.update.mock.calls.length;
      orientation(true, 24);
      expect(fixture.update).toHaveBeenCalledTimes(updates);
      fixture.page("pagehide", false);
      fixture.driver.finish();
      expect(fixture.free).toHaveBeenCalledOnce();
    } finally { fixture.page("pagehide", false); fixture.driver.finish(); await fixture.browser.happyDOM.abort(); }
  }, 20_000);

  it("retries initial observation failure, retains preparation through BFCache and requests permission inside the click", async () => {
    const diagnostic = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const observationFailure = new Error("Initial renderer observation failed");
    const cleanupFailure = new Error("Initial renderer cleanup failed");
    capture.observation = vi.fn().mockImplementationOnce(() => {
      capture.driver?.dispose.mockImplementationOnce(() => { throw cleanupFailure; });
      throw observationFailure;
    });
    const fixture = await browserFixture("permission");
    let driver = fixture.driver;
    try {
      expect(driver.dispose).toHaveBeenCalledOnce();
      expect(fixture.mount.querySelector(".presentation-canvas")).toBeNull();
      const failures = fixture.update.mock.calls.map(([json]) => (JSON.parse(json) as ApplicationObservation).input)
        .filter(input => input.kind === "renderer_created" && input.result.kind === "failed");
      expect(failures).toHaveLength(1);
      const failure = failures[0];
      if (failure?.kind !== "renderer_created" || failure.result.kind !== "failed") throw new Error("Missing renderer failure");
      expect(failure.result.cause).toContain(String(observationFailure));
      expect(failure.result.cause).toContain(String(cleanupFailure));
      expect(diagnostic).toHaveBeenCalledWith(failure.result.cause);
      fixture.mount.querySelector<HTMLButtonElement>('[data-control-id="game-boot-retry"]')?.click();
      if (capture.driver === null) throw new Error("Retry did not create a renderer");
      expect(capture.driver).not.toBe(driver);
      driver = capture.driver;
      expect(driver.requests).toHaveLength(1);
      fixture.page("pagehide", true);
      driver.finish();
      expect(fixture.free).not.toHaveBeenCalled();
      expect(driver.dispose).not.toHaveBeenCalled();
      fixture.page("pageshow", true);
      driver.finish();
      let inClick = false;
      const permission = vi.fn(() => { expect(inClick).toBe(true); });
      capture.permission = permission;
      for (const id of ["boot-enter-phone-vr", "boot-enter-hmd"]) {
        const choice = fixture.mount.querySelector<HTMLButtonElement>(`[data-control-id="${id}"]`);
        if (choice === null || choice.disabled) throw new Error("Retained Title is not interactive");
        inClick = true;
        choice.click();
        inClick = false;
      }
      expect(permission).toHaveBeenCalledTimes(2);
      const start = fixture.mount.querySelector<HTMLButtonElement>('[data-control-id="game-title-start"]');
      if (start === null) throw new Error("Permission failure did not restore Title controls");
      start.click();
      expect(fixture.mount.querySelector('[data-control-id="game-setup-start"]')).not.toBeNull();
      fixture.page("pagehide", false);
      driver.finish();
      expect(driver.dispose).toHaveBeenCalledOnce();
      expect(fixture.free).toHaveBeenCalledOnce();
      const updates = fixture.update.mock.calls.length;
      fixture.browser.document.documentElement.style.fontSize = "20px";
      fixture.page("pageshow", true);
      await Promise.resolve();
      expect(fixture.update).toHaveBeenCalledTimes(updates);
    } finally { fixture.page("pagehide", false); driver.finish(); await fixture.browser.happyDOM.abort(); }
  }, 20_000);

  it.each(["cancelled preparation", "missing prepared handle"])("frees once after the actual %s terminal and every cleanup", async failure => {
    const fixture = await browserFixture(failure === "cancelled preparation" ? "probe" : null);
    const cleanup = vi.spyOn(BrowserFlightLogDownload.prototype, "dispose").mockImplementation(() => { throw new Error("download cleanup failed"); });
    const diagnostic = vi.spyOn(console, "error").mockImplementation(() => undefined);
    const remove = fixture.browser.removeEventListener.bind(fixture.browser);
    let sensorDetachFailed = false;
    const removals = vi.spyOn(fixture.browser, "removeEventListener").mockImplementation((...args) => {
      if (args[0] === "deviceorientation") {
        if (!sensorDetachFailed) { sensorDetachFailed = true; throw new Error("sensor detach failed"); }
        const updates = fixture.update.mock.calls.length;
        fixture.browser.dispatchEvent(new fixture.browser.Event("deviceorientation"));
        expect(fixture.update).toHaveBeenCalledTimes(updates);
      }
      Reflect.apply(remove, fixture.browser, args);
    });
    try {
      if (failure === "missing prepared handle") {
        fixture.driver.finish(true);
        expect(fixture.update.mock.calls.some(([json]) => json.includes('"binding_lost"'))).toBe(true);
      } else {
        fixture.driver.finish();
        const preparations = fixture.driver.requests.length;
        fixture.browser.innerWidth = 960;
        fixture.browser.dispatchEvent(new fixture.browser.Event("resize"));
        expect(fixture.driver.requests.length).toBeGreaterThan(preparations);
      }
      fixture.page("pagehide", false);
      if (failure === "cancelled preparation") expect(fixture.free).not.toHaveBeenCalled();
      fixture.driver.finish();
      expect(fixture.driver.dispose).toHaveBeenCalledOnce();
      expect(cleanup).toHaveBeenCalledOnce();
      expect(fixture.free).toHaveBeenCalledOnce();
      expect(diagnostic).toHaveBeenCalledWith(expect.any(AggregateError));
      if (failure === "cancelled preparation") {
        expect(fixture.update.mock.calls.some(([json]) => json.includes('"phone_sensors_stopped"') && json.includes("sensor detach failed"))).toBe(true);
        expect(removals.mock.calls.filter(([type]) => type === "deviceorientation")).toHaveLength(2);
      }
      fixture.page("pagehide", false);
      expect(fixture.free).toHaveBeenCalledOnce();
    } finally { await fixture.browser.happyDOM.abort(); }
  });
});
