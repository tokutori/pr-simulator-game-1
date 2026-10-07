import { readFileSync } from "node:fs";
import { IDBFactory } from "fake-indexeddb";
import { Window as BrowserWindow } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RendererAdapter } from "../../web/src/render/contracts/runtime.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";

const teardowns: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const teardown of teardowns.splice(0)) await teardown();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.doUnmock("../../web/src/render/engines/three/three-renderer.js");
});

async function fixture(failInitialization = false, seedLegacyArchive = false, deferScreenStart = false) {
  vi.resetModules();
  const browser = new BrowserWindow({ url: "http://localhost/" });
  const documentRef = browser.document as unknown as Document;
  browser.document.body.innerHTML = '<main id="app"></main>';
  browser.document.documentElement.lang = "ja";
  Object.defineProperty(browser.document, "fonts", { value: Object.assign(new browser.EventTarget(), { status: "loaded", ready: Promise.resolve() }) });
  vi.stubGlobal("window", browser);
  vi.stubGlobal("document", browser.document);
  vi.stubGlobal("navigator", browser.navigator);
  vi.stubGlobal("HTMLElement", browser.HTMLElement);
  vi.stubGlobal("HTMLInputElement", browser.HTMLInputElement);
  vi.stubGlobal("HTMLSelectElement", browser.HTMLSelectElement);
  const persistence = new IDBFactory();
  Object.defineProperty(browser, "indexedDB", { value: persistence });
  vi.stubGlobal("indexedDB", persistence);
  vi.spyOn(browser.HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
  const countdown: (() => void)[] = [];
  const timeout = browser.setTimeout.bind(browser);
  vi.spyOn(browser, "setTimeout").mockImplementation((handler, delay, ...args) => {
    if (delay === 800 && typeof handler === "function") {
      const callback = handler as (...values: unknown[]) => void;
      countdown.push(() => { callback(...args); });
      const handle = timeout(() => undefined, 0);
      browser.clearTimeout(handle);
      return handle;
    }
    return timeout(handler, delay, ...args);
  });
  let frame: Parameters<RendererAdapter["startLoop"]>[0] = () => { throw new Error("Renderer loop is not initialized"); };
  const renderer = {
    startLoop: vi.fn<RendererAdapter["startLoop"]>((callback) => {
      if (failInitialization) throw new Error("Injected renderer start failure");
      frame = callback;
    }),
    beginViewFrame: vi.fn(), stopLoop: vi.fn(), render: vi.fn(),
    setFlightPose: vi.fn<RendererAdapter["setFlightPose"]>(), setLakeVisualCondition: vi.fn(),
    setFlightCameraMode: vi.fn(), setCinematicCameraView: vi.fn(), transformTrackingPose: vi.fn<RendererAdapter["transformTrackingPose"]>((pose) => pose),
    resize: vi.fn(), setStereoPresentation: vi.fn(), setSelectRayHandler: vi.fn(),
    dispose: vi.fn(() => { if (failInitialization) throw new Error("Injected renderer cleanup failure"); })
  } satisfies RendererAdapter;
  vi.doMock("../../web/src/render/engines/three/three-renderer.js", () => ({
    createThreeRenderer: () => ({ renderer, webxr: { checkAvailability: () => Promise.resolve({ supported: false, message: "Headless WebXR is unavailable" }) } })
  }));
  const wasm = await import("../../web/pkg/birdman_game_wasm.js");
  wasm.initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
  if (seedLegacyArchive) {
    const { FlightRecordRepository, IndexedDbFlightRecordPersistence } = await import("../../web/src/game/flight-record-store.js");
    const { createArchivedPersonalBestSelection } = await import("../../web/src/game/archived-personal-best.js");
    const legacy = new wasm.GameSessionBridge(0);
    try {
      legacy.open_setup();
      legacy.prepare();
      legacy.mark_briefing_ready();
      legacy.start_countdown(1);
      legacy.advance_countdown();
      legacy.launch();
      for (let tick = 0; tick < 5; tick += 1) legacy.advance_tick(0, 0, 0, 0);
      legacy.abort();
      const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(persistence), () => new Date("2026-10-08T00:00:00Z"), createArchivedPersonalBestSelection);
      await repository.saveFrom(legacy);
    } finally { legacy.free(); }
  }
  const free = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "free");
  const exportRecord = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "export_flight_record_json");
  const summary = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "flight_record_summary_json");
  const abort = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "abort");
  const tick = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "advance_tick_json");
  const { PresentationRuntime } = await import("../../web/src/presentation/runtime.js");
  const runtimeDispose = vi.spyOn(PresentationRuntime.prototype, "dispose");
  let releaseInitialization: () => void = () => undefined;
  let initializationPaused = false;
  if (deferScreenStart) {
    const gate = new Promise<void>((resolve) => { releaseInitialization = resolve; });
    const { ScreenPresentationBackend } = await import("../../web/src/presentation/screen-backend.js");
    vi.spyOn(ScreenPresentationBackend.prototype, "start").mockImplementation(() => {
      initializationPaused = true;
      return gate;
    });
  }
  await import("../../web/src/main.js");
  const scene = () => documentRef.querySelector<HTMLElement>(".screen-ui-shell")?.dataset.scene;
  const page = (type: "pagehide" | "pageshow", persisted: boolean): void => {
    const event = new browser.Event(type);
    Object.defineProperty(event, "persisted", { value: persisted });
    browser.dispatchEvent(event);
  };
  teardowns.push(async () => {
    page("pagehide", false);
    await browser.happyDOM.abort();
  });
  const click = (id: string): void => {
    const button = documentRef.querySelector<HTMLButtonElement>(`button[data-control-id="${id}"]`);
    if (button === null || button.disabled) throw new Error(`Active control is missing: ${id}; scene=${String(scene())}; controls=${[...documentRef.querySelectorAll<HTMLButtonElement>("button[data-control-id]")].map((entry) => `${String(entry.dataset.controlId)}:${String(entry.disabled)}`).join(",")}`);
    button.click();
  };
  const launch = (): void => {
    click("game-briefing-start");
    let count = 0;
    while (countdown.length > 0 && count < 10) { countdown.shift()?.(); count += 1; }
    expect(scene()).toBe("Flight");
  };
  return { browser, documentRef, renderer, scene, click, launch, page, free, exportRecord, summary, abort, tick,
    runtimeDispose, releaseInitialization, initializationPaused: () => initializationPaused,
    frame: (timestamp: number) => { frame(timestamp, unavailableViewerFrame("not-stereo")); } };
}

function runFlightToResult(trial: Awaited<ReturnType<typeof fixture>>, initialTimestampMilliseconds: number,
  maximumElapsedMilliseconds: number): void {
  const frameMilliseconds = 1_000 / 30;
  for (let elapsed = frameMilliseconds; elapsed < maximumElapsedMilliseconds && trial.scene() === "Flight"; elapsed += frameMilliseconds) {
    trial.frame(initialTimestampMilliseconds + elapsed);
  }
  if (trial.scene() === "Flight") trial.frame(initialTimestampMilliseconds + maximumElapsedMilliseconds);
}

describe("public main entrypoint with actual two-tail Rust WASM", () => {
  it("connects explicit Setup, flight, Result, Retry, named Replay and Attract without disposing the Rust owner", async () => {
    const trial = await fixture(false, true);
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    trial.click("game-setup-select-information-0");
    trial.click("game-setup-select-weather-2");
    expect(trial.browser.document.body.textContent).toContain("気象による発進風速の上限・下限");
    trial.click("game-setup-start");
    expect(trial.scene()).toBe("Briefing");
    expect(trial.documentRef.querySelector('[data-control-id="game-briefing-technical"]')?.getAttribute("aria-expanded")).toBe("false");
    expect(trial.browser.document.body.textContent).toContain("独立したroll入力はない");
    trial.launch();
    trial.frame(0);
    trial.frame(10);
    expect(trial.renderer.setFlightPose.mock.calls.at(-1)?.[0]?.controls?.layout).toBe("tail_incidence");
    trial.click("game-flight-abort");
    expect(trial.scene()).toBe("Result");
    await vi.waitFor(() => { expect(trial.exportRecord).toHaveBeenCalledTimes(1); });
    const stored: unknown = JSON.parse(trial.exportRecord.mock.results[0]?.value as string);
    expect(stored).toMatchObject({ schema_version: 6, finalization: { reason: "manual_abort" } });
    trial.click("game-result-replay");
    expect(trial.scene()).toBe("Replay");
    trial.click("game-replay-return");
    expect(trial.scene()).toBe("Result");
    trial.click("game-result-retry");
    trial.launch();
    trial.frame(20);
    runFlightToResult(trial, 20, 199_990);
    expect(trial.scene()).toBe("Result");
    expect(trial.exportRecord).toHaveBeenCalledTimes(2);
    expect(JSON.parse(trial.exportRecord.mock.results[1]?.value as string)).toMatchObject({ schema_version: 6, finalization: { reason: "water_contact" } });
    await vi.waitFor(() => { expect(trial.browser.document.body.textContent).toContain("FlightRecord 3を保存した"); });
    expect(trial.free).not.toHaveBeenCalled();
    trial.click("game-result-title");
    trial.click("game-title-demo");
    expect(trial.scene()).toBe("Title");
    expect(trial.documentRef.querySelector('[data-control-id="game-attract-return"]')).not.toBeNull();
    expect(trial.renderer.setFlightPose.mock.calls.at(-1)?.[0]?.controls?.layout).toBe("tail_incidence");
    expect(trial.free).not.toHaveBeenCalled();
  });

  it("preserves the original initialization failure and frees the owner even if renderer cleanup throws", async () => {
    const trial = await fixture(true);
    await vi.waitFor(() => { expect(trial.browser.document.body.textContent).toContain("Injected renderer start failure"); });
    expect(trial.browser.document.body.textContent).toContain("Injected renderer cleanup failure");
    expect(trial.free).toHaveBeenCalledTimes(1);
    expect(trial.renderer.dispose).toHaveBeenCalledTimes(1);
  });

  it("disposes the initialized runtime owner once when pagehide races with deferred backend start", async () => {
    const trial = await fixture(false, false, true);
    await vi.waitFor(() => { expect(trial.initializationPaused()).toBe(true); });
    trial.page("pagehide", false);
    expect(trial.free).toHaveBeenCalledTimes(1);
    expect(trial.renderer.dispose).not.toHaveBeenCalled();
    trial.releaseInitialization();
    await vi.waitFor(() => { expect(trial.runtimeDispose).toHaveBeenCalledTimes(2); });
    await Promise.all(trial.runtimeDispose.mock.results.map((result) => result.value as Promise<unknown>));
    expect(trial.renderer.dispose).toHaveBeenCalledTimes(1);
    expect(trial.free).toHaveBeenCalledTimes(1);
    expect(trial.browser.document.body.textContent).toContain("Page hidden");
  });

  it("opens a legacy saved snapshot through the Tail owner and preserves its explicit three-axis render layout", async () => {
    const trial = await fixture(false, true);
    await vi.waitFor(() => { expect(trial.documentRef.querySelector('[data-control-id="game-title-open-record-1"]')).not.toBeNull(); });
    trial.click("game-title-open-record-1");
    await vi.waitFor(() => { expect(trial.scene()).toBe("Replay"); });
    expect(trial.renderer.setFlightPose.mock.calls.at(-1)?.[0]?.controls?.layout).toBe("legacy_three_axis");
    expect(trial.exportRecord).not.toHaveBeenCalled();
    trial.click("game-replay-view-mode");
    trial.click("game-replay-speed-2");
    trial.click("game-replay-return");
    expect(trial.scene()).toBe("Title");
    trial.click("game-title-start");
    trial.click("game-setup-start");
    trial.launch();
    expect(trial.renderer.setFlightPose.mock.calls.at(-1)?.[0]?.controls?.layout).toBe("tail_incidence");
    expect(trial.free).not.toHaveBeenCalled();
  });

  it("publishes the initial Rust snapshot and Abort when controller creation and input cleanup both fail", async () => {
    const trial = await fixture();
    const { BrowserTailPilotInput } = await import("../../web/src/game/browser-tail-input.js");
    vi.spyOn(BrowserTailPilotInput.prototype, "dispose").mockImplementationOnce(() => { throw new Error("Injected input cleanup failure"); });
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    trial.click("game-setup-start");
    let failCreate = true;
    trial.renderer.setFlightPose.mockImplementation((pose) => {
      if (pose !== null && failCreate) { failCreate = false; throw new Error("Injected controller creation failure"); }
    });
    trial.launch();
    expect(trial.browser.document.body.textContent).toContain("Injected controller creation failure");
    expect(trial.browser.document.body.textContent).toContain("Injected input cleanup failure");
    expect(trial.browser.document.body.textContent).toContain("Last valid · tick 0");
    trial.click("game-flight-abort");
    expect(trial.scene()).toBe("Result");
    expect(trial.free).not.toHaveBeenCalled();
  });

  it.each(["water_contact", "manual_abort"] as const)("delivers Result persistence and Analysis despite a %s committed-view failure", async (reason) => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    trial.click("game-setup-start");
    trial.launch();
    let injected = false;
    trial.renderer.setLakeVisualCondition.mockImplementation(() => {
      const terminal = reason === "manual_abort" ? trial.abort.mock.results.length > 0
        : trial.tick.mock.results.some((result) => {
          const snapshot: unknown = JSON.parse(result.value as string);
          return typeof snapshot === "object" && snapshot !== null && "phase_code" in snapshot && snapshot.phase_code === 7;
        });
      if (terminal && !injected) { injected = true; throw new Error("Injected committed Result view failure"); }
    });
    trial.frame(0);
    if (reason === "manual_abort") trial.click("game-flight-abort");
    else runFlightToResult(trial, 0, 200_000);
    expect(injected).toBe(true);
    expect(trial.scene()).toBe("Result");
    expect(trial.browser.document.body.textContent).toContain("Injected committed Result view failure");
    expect(trial.exportRecord).toHaveBeenCalledTimes(1);
    expect(trial.summary).toHaveBeenCalled();
    expect(JSON.parse(trial.exportRecord.mock.results[0]?.value as string)).toMatchObject({ schema_version: 6, finalization: { reason } });
    await vi.waitFor(() => { expect(trial.browser.document.body.textContent).toContain("FlightRecord 1を保存した"); });
    trial.click("game-result-replay");
    expect(trial.scene()).toBe("Replay");
    expect(trial.renderer.setFlightPose.mock.calls.at(-1)?.[0]?.controls?.layout).toBe("tail_incidence");
    expect(trial.exportRecord).toHaveBeenCalledTimes(1);
  });

  it("retains the same flight owner across BFCache and exposes Abort after a Retry reset failure", async () => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    trial.click("game-setup-start");
    trial.launch();
    trial.frame(0);
    trial.frame(10);
    trial.page("pagehide", true);
    expect(trial.free).not.toHaveBeenCalled();
    trial.page("pageshow", true);
    await vi.waitFor(() => { expect(trial.documentRef.querySelector<HTMLButtonElement>('button[data-control-id="game-flight-resume"]')?.disabled).toBe(false); });
    trial.click("game-flight-resume");
    trial.click("game-flight-abort");
    trial.click("game-result-retry");
    let failReset = true;
    trial.renderer.setFlightPose.mockImplementation((pose) => {
      if (pose !== null && failReset) { failReset = false; throw new Error("Injected Retry reset pose failure"); }
    });
    trial.launch();
    expect(trial.browser.document.body.textContent).toContain("Injected Retry reset pose failure");
    expect(trial.browser.document.body.textContent).toContain("Last valid · tick 0");
    expect(trial.documentRef.querySelector('[data-control-id="game-flight-pause"]')).toBeNull();
    expect(trial.documentRef.querySelector('[data-control-id="game-flight-resume"]')).toBeNull();
    trial.frame(20);
    trial.frame(100);
    expect(trial.browser.document.body.textContent).toContain("Last valid · tick 0");
    trial.click("game-flight-abort");
    expect(trial.scene()).toBe("Result");
    expect(trial.free).not.toHaveBeenCalled();
  });
});
