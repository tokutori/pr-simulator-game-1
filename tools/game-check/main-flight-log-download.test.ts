import { readFileSync } from "node:fs";
import { Window as BrowserWindow } from "happy-dom";
import { indexedDB, IDBKeyRange } from "fake-indexeddb";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RendererAdapter } from "../../web/src/render/contracts/runtime.js";

const teardowns: (() => Promise<void>)[] = [];

afterEach(async () => {
  for (const teardown of teardowns.splice(0)) await teardown();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.doUnmock("../../web/src/render/engines/three/three-renderer.js");
  vi.doUnmock("../../web/src/app/session-factory.js");
});

async function fixture(archive: false | "tail" | "legacy" = false) {
  vi.resetModules();
  const browser = new BrowserWindow({ url: "http://localhost/" });
  browser.document.body.innerHTML = '<main id="app"></main>';
  vi.stubGlobal("window", browser);
  vi.stubGlobal("document", browser.document);
  vi.stubGlobal("navigator", browser.navigator);
  vi.stubGlobal("HTMLElement", browser.HTMLElement);
  vi.stubGlobal("HTMLInputElement", browser.HTMLInputElement);
  vi.stubGlobal("HTMLSelectElement", browser.HTMLSelectElement);
  Object.defineProperty(browser.document, "fonts", { value: Object.assign(new browser.EventTarget(), { status: "loaded", ready: Promise.resolve() }) });
  vi.stubGlobal("indexedDB", indexedDB);
  vi.stubGlobal("IDBKeyRange", IDBKeyRange);
  const blobs: Blob[] = [];
  const createUrl = vi.spyOn(URL, "createObjectURL").mockImplementation((blob) => {
    if (!(blob instanceof Blob)) throw new Error("Download must contain a Blob");
    blobs.push(blob);
    return `blob:flight-log-${String(blobs.length)}`;
  });
  const revokeUrl = vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => undefined);
  const click = vi.spyOn(browser.HTMLAnchorElement.prototype, "click").mockImplementation(() => undefined);
  const renderer = {
    startLoop: vi.fn(), beginViewFrame: vi.fn(), stopLoop: vi.fn(), render: vi.fn(), setFlightPose: vi.fn(), setPreparedFlightPose: vi.fn(),
    setLakeVisualCondition: vi.fn(), setLakeSkyCondition: vi.fn(), setFlightCameraMode: vi.fn(), setCinematicCameraView: vi.fn(),
    transformTrackingPose: vi.fn<RendererAdapter["transformTrackingPose"]>((pose) => pose),
    resize: vi.fn(), setStereoPresentation: vi.fn(), setSelectRayHandler: vi.fn(), dispose: vi.fn()
  } satisfies RendererAdapter;
  vi.doMock("../../web/src/render/engines/three/three-renderer.js", () => ({
    createThreeRenderer: () => ({ renderer, webxr: { checkAvailability: () => Promise.resolve({ supported: false, message: "Headless WebXR is unavailable" }) } })
  }));
  const wasm = await import("../../web/pkg/birdman_game_wasm.js");
  wasm.initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
  const session = new wasm.HybridGameSessionBridge(0, 0x55aa, 0x5f98);
  session.open_setup();
  session.prepare();
  session.mark_briefing_ready();
  session.start_countdown(1);
  session.advance_countdown();
  session.launch();
  const { encodeTailLogicalInput } = await import("../../web/src/game/tail-session-codec.js");
  session.advance_tick_json(encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
    desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } }));
  session.abort();
  let original = ` \n${session.export_flight_record_json()}\n `;
  if (archive === "legacy") {
    const legacy = new wasm.GameSessionBridge(0);
    try {
      legacy.open_setup();
      legacy.prepare();
      legacy.mark_briefing_ready();
      legacy.start_countdown(1);
      legacy.advance_countdown();
      legacy.launch();
      legacy.advance_tick(0, 0, 0, 0);
      legacy.abort();
      original = ` \n${legacy.export_flight_record_json()}\n `;
    } finally { legacy.free(); }
  }
  if (archive) {
    session.return_to_title();
    session.open_archived_flight_record(original);
  }
  const { TailAppSessionFacade } = await import("../../web/src/app/session-facade.js");
  const facade = new TailAppSessionFacade(session, wasm.physics_hz());
  const free = vi.spyOn(session, "free");
  vi.doMock("../../web/src/app/session-factory.js", () => ({ initializeAppSession: () => Promise.resolve(facade) }));
  const pageEvent = (type: "pagehide" | "pageshow", persisted: boolean): void => {
    const event = new browser.Event(type);
    Object.defineProperty(event, "persisted", { value: persisted });
    browser.dispatchEvent(event);
  };
  const pageHide = (persisted: boolean): void => { pageEvent("pagehide", persisted); };
  teardowns.push(async () => {
    pageHide(false);
    await browser.happyDOM.abort();
  });
  await import("../../web/src/main.js");
  await vi.waitFor(() => { expect(browser.document.querySelector('[data-control-id="game-flight-log-csv"]')).not.toBeNull(); });
  const button = (id: string): HTMLButtonElement => {
    const element = browser.document.querySelector(`[data-control-id="${id}"]`);
    if (!(element instanceof browser.HTMLButtonElement)) throw new Error(`Required button ${id} is unavailable`);
    return element as unknown as HTMLButtonElement;
  };
  return { browser, renderer, session, facade, original, blobs, createUrl, revokeUrl, click, button, pageHide, free,
    pageRestore: () => { pageEvent("pageshow", true); } };
}

describe("actual Tail main, facade and generated WASM flight log download", () => {
  it("downloads Rust CSV and JSON for Result without changing the record or phase", async () => {
    const trial = await fixture();
    const before = trial.session.export_current_flight_record_json();
    const expectedCsv = trial.session.export_flight_log_csv();
    const csv = vi.spyOn(trial.session, "export_flight_log_csv");
    const json = vi.spyOn(trial.session, "export_current_flight_record_json");
    trial.button("game-flight-log-csv").click();
    trial.button("game-flight-log-json").click();
    expect(csv).toHaveBeenCalledTimes(1);
    expect(json).toHaveBeenCalledTimes(1);
    expect(await trial.blobs[0]?.text()).toBe(expectedCsv);
    expect(await trial.blobs[1]?.text()).toBe(before);
    expect(trial.session.phase_code()).toBe(7);
    expect(trial.session.export_current_flight_record_json()).toBe(before);
    expect(trial.browser.document.body.textContent).toContain("ダウンロードを要求した");
    expect(trial.revokeUrl).not.toHaveBeenCalled();
    trial.button("game-result-open-analysis").click();
    expect(trial.button("game-flight-log-csv").disabled).toBe(false);
    trial.pageHide(true);
    expect(trial.free).not.toHaveBeenCalled();
    expect(trial.revokeUrl).not.toHaveBeenCalled();
    trial.pageRestore();
    await vi.waitFor(() => { expect(trial.button("game-flight-log-json").disabled).toBe(false); });
    trial.pageHide(false);
    expect(trial.revokeUrl.mock.calls).toEqual([["blob:flight-log-1"], ["blob:flight-log-2"]]);
    expect(trial.free).toHaveBeenCalledTimes(1);
  });

  it.each(["tail", "legacy"] as const)("preserves the selected %s archive original JSON and playback clock during Analysis export", async (layout) => {
    const trial = await fixture(layout);
    trial.button("game-replay-view-mode").click();
    trial.button("game-replay-view-mode").click();
    const clock = trial.session.playback_clock_state();
    trial.button("game-flight-log-json").click();
    trial.button("game-flight-log-csv").click();
    expect(await trial.blobs[0]?.text()).toBe(trial.original);
    expect(trial.session.playback_clock_state()).toEqual(clock);
    expect(trial.session.phase_code()).toBe(9);
    expect(trial.session.is_archived_replay()).toBe(true);
    expect(trial.browser.document.body.textContent).toContain("finite-difference推定値");
    const csv = trial.session.export_flight_log_csv();
    expect(() => { trial.session.open_archived_flight_record("{invalid archive}"); }).toThrow();
    expect(trial.session.export_current_flight_record_json()).toBe(trial.original);
    expect(trial.session.export_flight_log_csv()).toBe(csv);
    expect(() => trial.session.export_flight_record_json()).toThrow();
  });

  it("reports Rust and browser failures and permits a later successful request", async () => {
    const trial = await fixture();
    vi.spyOn(trial.session, "export_flight_log_csv").mockImplementationOnce(() => { throw new Error("Injected Rust export failure"); });
    trial.button("game-flight-log-csv").click();
    expect(trial.browser.document.body.textContent).toContain("Injected Rust export failure");
    expect(trial.createUrl).not.toHaveBeenCalled();
    trial.click.mockImplementationOnce(() => { throw new Error("Injected browser download failure"); });
    trial.button("game-flight-log-json").click();
    expect(trial.browser.document.body.textContent).toContain("Injected browser download failure");
    expect(trial.revokeUrl).toHaveBeenCalledExactlyOnceWith("blob:flight-log-1");
    expect(trial.browser.document.querySelector("a[download]")).toBeNull();
    trial.button("game-flight-log-json").click();
    expect(trial.browser.document.body.textContent).toContain("ダウンロードを要求した");
  });

  it("rejects a read whose source changes through Retry before browser activation", async () => {
    const trial = await fixture();
    vi.spyOn(trial.session, "export_flight_log_csv").mockImplementationOnce(() => {
      trial.button("game-result-retry").click();
      return "stale source";
    });
    trial.button("game-flight-log-csv").click();
    expect(trial.createUrl).not.toHaveBeenCalled();
    expect(trial.session.phase_code()).toBe(3);
    expect(trial.browser.document.body.textContent).not.toContain("ダウンロードを要求した");
  });

  it.each([2, 3])("delivers one export effect and preserves presentation exception on render %s", async (failedRender) => {
    const trial = await fixture();
    const failure = new Error("Injected committed download view failure");
    const errors: unknown[] = [];
    trial.browser.addEventListener("error", (event) => {
      if (event instanceof trial.browser.ErrorEvent) {
        const error: unknown = event.error;
        errors.push(error);
      }
      event.preventDefault();
    });
    let renders = 0;
    trial.renderer.setLakeVisualCondition.mockImplementation(() => {
      renders += 1;
      if (renders === failedRender) throw failure;
    });
    const csv = vi.spyOn(trial.session, "export_flight_log_csv");
    try { trial.button("game-flight-log-csv").click(); }
    catch (error: unknown) { errors.push(error); }
    expect(csv).toHaveBeenCalledTimes(1);
    expect(trial.createUrl).toHaveBeenCalledTimes(1);
    expect(errors).toContain(failure);
    if (failedRender === 2) expect(trial.browser.document.body.textContent).toContain("ダウンロードを要求した");
    else trial.button("game-result-open-analysis").click();
    expect(trial.button("game-flight-log-json").disabled).toBe(false);
    trial.button("game-flight-log-json").click();
    expect(trial.createUrl).toHaveBeenCalledTimes(2);
    expect(trial.session.phase_code()).toBe(7);
  });

  it("rejects native JS export error paths outside Result and Replay without changing their phase", async () => {
    const trial = await fixture();
    const assertRejected = (phaseCode: number): void => {
      expect(trial.session.phase_code()).toBe(phaseCode);
      expect(() => trial.session.export_current_flight_record_json()).toThrow();
      expect(() => trial.session.export_flight_log_csv()).toThrow();
      expect(trial.session.phase_code()).toBe(phaseCode);
    };
    trial.session.return_to_title();
    assertRejected(0);
    trial.session.enter_attract();
    assertRejected(10);
    trial.session.leave_attract();
    trial.session.open_setup();
    assertRejected(1);
    trial.session.prepare();
    assertRejected(2);
    trial.session.mark_briefing_ready();
    assertRejected(3);
    trial.session.start_countdown(1);
    assertRejected(4);
    trial.session.advance_countdown();
    trial.session.launch();
    assertRejected(5);
    trial.session.pause(0);
    assertRejected(6);
  });
});
