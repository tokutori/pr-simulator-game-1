import { readFileSync } from "node:fs";
import { IDBFactory } from "fake-indexeddb";
import { Window as BrowserWindow } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { RendererAdapter } from "../../web/src/render/contracts/runtime.js";

const teardowns: (() => Promise<void>)[] = [];

afterEach(async () => {
  for (const teardown of teardowns.splice(0)) await teardown();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.doUnmock("../../web/src/render/engines/three/three-renderer.js");
});

async function fixture(options: { readonly deferScreenStart?: boolean; readonly failStart?: boolean; readonly failCleanup?: boolean } = {}) {
  vi.resetModules();
  const browser = new BrowserWindow({ url: "http://localhost/" });
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
  const renderer = {
    startLoop: vi.fn<RendererAdapter["startLoop"]>(() => {
      if (options.failStart === true) throw new Error("Injected renderer start failure");
    }),
    beginViewFrame: vi.fn(), stopLoop: vi.fn(), render: vi.fn(), setFlightPose: vi.fn(), setPreparedFlightPose: vi.fn(),
    setLakeVisualCondition: vi.fn(), setLakeSkyCondition: vi.fn(), setFlightCameraMode: vi.fn(), setCinematicCameraView: vi.fn(),
    transformTrackingPose: vi.fn<RendererAdapter["transformTrackingPose"]>((pose) => pose),
    resize: vi.fn(), setStereoPresentation: vi.fn(), setSelectRayHandler: vi.fn(),
    dispose: vi.fn(() => {
      if (options.failCleanup === true) throw new Error("Injected renderer cleanup failure");
    })
  } satisfies RendererAdapter;
  vi.doMock("../../web/src/render/engines/three/three-renderer.js", () => ({
    createThreeRenderer: () => ({
      renderer,
      webxr: { checkAvailability: () => Promise.resolve({ supported: false, message: "Headless WebXR is unavailable" }) }
    })
  }));
  const wasm = await import("../../web/pkg/birdman_game_wasm.js");
  wasm.initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
  const free = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "free");
  const { TailAppSessionFacade } = await import("../../web/src/app/session-facade.js");
  const facadeDispose = vi.spyOn(TailAppSessionFacade.prototype, "dispose");
  const { PresentationRuntime } = await import("../../web/src/presentation/runtime.js");
  const runtimeDispose = vi.spyOn(PresentationRuntime.prototype, "dispose");
  let releaseInitialization: () => void = () => undefined;
  let initializationPaused = false;
  if (options.deferScreenStart === true) {
    const gate = new Promise<void>((resolve) => { releaseInitialization = resolve; });
    const { ScreenPresentationBackend } = await import("../../web/src/presentation/screen-backend.js");
    vi.spyOn(ScreenPresentationBackend.prototype, "start").mockImplementation(() => {
      initializationPaused = true;
      return gate;
    });
  }
  const pageHide = (persisted: boolean): void => {
    const event = new browser.Event("pagehide");
    Object.defineProperty(event, "persisted", { value: persisted });
    browser.dispatchEvent(event);
  };
  teardowns.push(async () => {
    releaseInitialization();
    pageHide(false);
    await Promise.allSettled(runtimeDispose.mock.results.map((result) => result.value as Promise<unknown>));
    await browser.happyDOM.abort();
  });
  await import("../../web/src/main.js");
  return { browser, renderer, free, facadeDispose, runtimeDispose, pageHide, releaseInitialization,
    initializationPaused: () => initializationPaused,
    scene: () => browser.document.querySelector(".screen-ui-shell")?.getAttribute("data-scene") };
}

describe("Tail main startup resource ownership", () => {
  it("disposes the runtime and Rust session once when pagehide interrupts deferred Screen start", async () => {
    const trial = await fixture({ deferScreenStart: true });
    await vi.waitFor(() => { expect(trial.initializationPaused()).toBe(true); });
    trial.pageHide(false);
    expect(trial.facadeDispose).toHaveBeenCalledTimes(1);
    expect(trial.free).toHaveBeenCalledTimes(1);
    expect(trial.renderer.dispose).not.toHaveBeenCalled();
    trial.releaseInitialization();
    await vi.waitFor(() => { expect(trial.runtimeDispose).toHaveBeenCalledTimes(2); });
    await Promise.all(trial.runtimeDispose.mock.results.map((result) => result.value as Promise<unknown>));
    await vi.waitFor(() => { expect(trial.renderer.dispose).toHaveBeenCalledTimes(1); });
    expect(trial.free).toHaveBeenCalledTimes(1);
    expect(trial.scene()).toBe("Boot");
    expect(trial.browser.document.querySelector(".screen-ui-mount")?.hasAttribute("hidden")).toBe(true);
    expect(trial.browser.document.querySelector('button[data-control-id^="game-"]')).toBeNull();
  });

  it("retains ownership until non-BFCache teardown after normal Tail initialization", async () => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    expect(trial.renderer.dispose).not.toHaveBeenCalled();
    expect(trial.free).not.toHaveBeenCalled();
    expect(trial.facadeDispose).not.toHaveBeenCalled();
    trial.pageHide(false);
    await Promise.all(trial.runtimeDispose.mock.results.map((result) => result.value as Promise<unknown>));
    expect(trial.renderer.dispose).toHaveBeenCalledTimes(1);
    expect(trial.free).toHaveBeenCalledTimes(1);
    expect(trial.facadeDispose).toHaveBeenCalledTimes(1);
  });

  it("preserves the start failure and frees the Rust session despite renderer cleanup failure", async () => {
    const trial = await fixture({ failStart: true, failCleanup: true });
    await vi.waitFor(() => {
      expect(trial.browser.document.body.textContent).toContain("Injected renderer start failure");
    });
    expect(trial.browser.document.body.textContent).toContain("Injected renderer cleanup failure");
    expect(trial.renderer.dispose).toHaveBeenCalledTimes(1);
    expect(trial.free).toHaveBeenCalledTimes(1);
    expect(trial.facadeDispose).toHaveBeenCalledTimes(1);
  });
});
