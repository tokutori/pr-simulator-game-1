import { readFileSync } from "node:fs";
import { IDBFactory } from "fake-indexeddb";
import { Window as BrowserWindow } from "happy-dom";
import { afterEach, describe, expect, it, vi } from "vitest";
import { readContract } from "../game-ui-build/validate.js";
import type { GameUiView } from "../../web/src/generated/game-ui-view.js";
import type { LakeWaterQualityCleanupResult, RendererAdapter } from "../../web/src/render/contracts/runtime.js";
import { unavailableViewerFrame } from "../../web/src/render/contracts/viewer-frame.js";
import type { WebXrSessionPort } from "../../web/src/presentation/webxr-contracts.js";
import { sharedGameUiPlatform } from "./shared-game-ui-fixture.js";
import type { ScreenRenderAutoBinding } from "../../web/src/render/contracts/screen-render-auto.js";

const teardowns: (() => Promise<void>)[] = [];
afterEach(async () => {
  for (const teardown of teardowns.splice(0)) await teardown();
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  vi.doUnmock("../../web/src/render/engines/three/three-renderer.js");
});

async function fixture(failInitialization = false, seedCurrentArchive = false, deferScreenStart = false, captureReplayTicks = false,
  recordListMode: "normal" | "loading" | "failed" = "normal", webXrAvailable = false) {
  vi.resetModules();
  const browser = new BrowserWindow({ url: "http://localhost/" });
  const documentRef = browser.document as unknown as Document;
  browser.document.body.innerHTML = '<main id="app"></main>';
  browser.document.documentElement.lang = "ja";
  browser.document.documentElement.style.fontSize = "16px";
  Object.defineProperty(browser.document, "fonts", { value: Object.assign(new browser.EventTarget(), { status: "loaded", ready: Promise.resolve() }) });
  vi.stubGlobal("window", browser);
  vi.stubGlobal("document", browser.document);
  vi.stubGlobal("navigator", browser.navigator);
  vi.stubGlobal("HTMLElement", browser.HTMLElement);
  vi.stubGlobal("HTMLInputElement", browser.HTMLInputElement);
  vi.stubGlobal("HTMLSelectElement", browser.HTMLSelectElement);
  vi.spyOn(browser.document, "hasFocus").mockReturnValue(true);
  let nowMilliseconds = 0;
  vi.spyOn(performance, "now").mockImplementation(() => nowMilliseconds);
  const persistence = new IDBFactory();
  Object.defineProperty(browser, "indexedDB", { value: persistence });
  vi.stubGlobal("indexedDB", persistence);
  vi.spyOn(browser.HTMLCanvasElement.prototype, "getContext").mockReturnValue(null);
  const countdown: (() => void)[] = [];
  const replayTicks: (() => void)[] = [];
  const timeout = browser.setTimeout.bind(browser);
  vi.spyOn(browser, "setTimeout").mockImplementation((handler, delay, ...args) => {
    if ((delay === 1000 || captureReplayTicks && delay === 50) && typeof handler === "function") {
      const callback = handler as (...values: unknown[]) => void;
      (delay === 1000 ? countdown : replayTicks).push(() => { callback(...args); });
      const handle = timeout(() => undefined, 0);
      browser.clearTimeout(handle);
      return handle;
    }
    return timeout(handler, delay, ...args);
  });
  let frame: Parameters<RendererAdapter["startLoop"]>[0] = () => { throw new Error("Renderer loop is not initialized"); };
  let screenBinding: ScreenRenderAutoBinding | undefined;
  const acknowledgeScreenScale = (): void => {
    if (screenBinding === undefined) throw new Error("Missing shared Screen policy binding");
    const policy = screenBinding.policy;
    if (policy.requested_scale_percent() !== 0) {
      const quality = policy.requested_water_quality();
      if (quality !== "low" && quality !== "medium" && quality !== "high") throw new Error("Missing requested water quality");
      policy.applied(policy.requested_epoch(), policy.requested_scale_percent(), policy.requested_shadow_map_size(), quality, nowMilliseconds);
    }
    screenBinding.onSnapshot(screenBinding.snapshot());
  };
  const acknowledgeWaterQuality = (quality: Parameters<RendererAdapter["setLakeWaterQuality"]>[0]): void => {
    screenBinding?.policy.select_water_quality(quality, nowMilliseconds);
    acknowledgeScreenScale();
  };
  const renderer = {
    startLoop: vi.fn<RendererAdapter["startLoop"]>((callback) => {
      if (failInitialization) throw new Error("Injected renderer start failure");
      frame = callback;
    }),
    beginViewFrame: vi.fn(), stopLoop: vi.fn(), render: vi.fn(),
    setFlightPose: vi.fn<RendererAdapter["setFlightPose"]>(), setLakeVisualCondition: vi.fn(),
    setLakeWaterQuality: vi.fn<RendererAdapter["setLakeWaterQuality"]>((quality) => {
      acknowledgeWaterQuality(quality);
      return Promise.resolve({ kind: "complete" });
    }),
    setPreparedFlightPose: vi.fn<RendererAdapter["setPreparedFlightPose"]>(),
    setLakeSkyCondition: vi.fn<RendererAdapter["setLakeSkyCondition"]>(),
    setLakeVenueVisible: vi.fn<RendererAdapter["setLakeVenueVisible"]>(),
    setFlightCameraMode: vi.fn(), setCinematicCameraView: vi.fn(), transformTrackingPose: vi.fn<RendererAdapter["transformTrackingPose"]>((pose) => pose),
    resize: vi.fn(), setStereoPresentation: vi.fn(),
    setScreenRenderScale: vi.fn((factor: number) => { screenBinding?.policy.select_manual(factor * 100, nowMilliseconds); acknowledgeScreenScale(); }),
    setScreenRenderAuto: vi.fn((enabled: boolean) => { screenBinding?.policy.select_auto(enabled, nowMilliseconds); acknowledgeScreenScale(); }),
    setSelectRayHandler: vi.fn(),
    dispose: vi.fn(() => {
      screenBinding?.policy.free();
      screenBinding = undefined;
      if (failInitialization) throw new Error("Injected renderer cleanup failure");
    })
  } satisfies RendererAdapter;
  let sessionEndHandler: (() => void) | null = null;
  const webxr: WebXrSessionPort = {
    transformTrackingPose: (pose) => pose,
    checkAvailability: () => Promise.resolve({ supported: webXrAvailable, message: "Headless WebXR test port" }),
    requestSessionFromUserGesture: () => Promise.resolve({ ok: true }),
    startSession: () => Promise.resolve(), endSession: () => Promise.resolve(),
    setSessionEndHandler: (handler) => { sessionEndHandler = handler; },
    setReferenceSpaceResetHandler: () => undefined
  };
  vi.doMock("../../web/src/render/engines/three/three-renderer.js", () => ({
    createThreeRenderer: (...args: Parameters<typeof import("../../web/src/render/engines/three/three-renderer.js")["createThreeRenderer"]>) => {
      screenBinding = args[6];
      return { renderer, webxr };
    }
  }));
  const wasm = await import("../../web/pkg/birdman_game_wasm.js");
  wasm.initSync({ module: new Uint8Array(readFileSync(new URL("../../web/pkg/birdman_game_wasm_bg.wasm", import.meta.url))) });
  if (seedCurrentArchive) {
    const { FlightRecordRepository, IndexedDbFlightRecordPersistence } = await import("../../web/src/game/flight-record-store.js");
    const { createArchivedPersonalBestSelection } = await import("../../web/src/game/archived-personal-best.js");
    const archived = new wasm.HybridGameSessionBridge(0, 21, 22);
    try {
      archived.open_setup();
      archived.prepare();
      archived.mark_briefing_ready();
      archived.start_countdown(1);
      archived.observe_countdown(archived.countdown_token(), 1, true);
      const { encodeTailLogicalInput } = await import("../../web/src/game/tail-input.js");
      const neutral = encodeTailLogicalInput({ controlLayout: "tail_incidence", noseUp: 0, turnRight: 0,
        desiredPitchRateRadiansPerSecond: 0, desiredYawRateRadiansPerSecond: 0, pilotPositionCommand: { kind: "hold" } });
      for (let tick = 0; tick < 5; tick += 1) archived.advance_tick_json(neutral);
      archived.abort();
      const repository = new FlightRecordRepository(new IndexedDbFlightRecordPersistence(persistence, wasm.TailPersonalBestSelectionBridge.comparison_version()), () => new Date("2026-10-08T00:00:00Z"), createArchivedPersonalBestSelection);
      await repository.saveFrom(archived);
    } finally { archived.free(); }
  }
  if (recordListMode !== "normal") {
    const { FlightRecordRepository } = await import("../../web/src/game/flight-record-store.js");
    const list = vi.spyOn(FlightRecordRepository.prototype, "recent");
    if (recordListMode === "loading") list.mockImplementation(() => new Promise(() => undefined));
    else list.mockRejectedValue(new Error("Injected record listing failure"));
  }
  const free = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "free");
  const exportRecord = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "export_flight_record_json");
  const summary = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "flight_record_summary_json");
  const sharedCommand = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "apply_game_ui_session_command_json");
  const observeCountdown = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "observe_countdown");
  const pause = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "pause");
  const clearPauseReason = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "clear_pause_reason");
  const sharedView = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "game_ui_view_json");
  const tick = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "advance_tick_json");
  const advancePlayback = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "advance_playback");
  const playbackClock = vi.spyOn(wasm.HybridGameSessionBridge.prototype, "playback_clock_state");
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
  const factory = await import("../../web/src/app/session-factory.js");
  const initializeSession = vi.spyOn(factory, "initializeAppSession");
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
    frame(nowMilliseconds, unavailableViewerFrame("not-stereo"));
    click("game-briefing-start");
    let count = 0;
    while (countdown.length > 0 && count < 10) { countdownTick(); count += 1; }
    expect(scene()).toBe("Flight");
  };
  const countdownTick = (elapsedMilliseconds = 1000, callback = countdown.shift(), framesAvailable = true): void => {
    const nextMilliseconds = nowMilliseconds + elapsedMilliseconds;
    if (framesAvailable) {
      while (nowMilliseconds < nextMilliseconds) {
        nowMilliseconds = Math.min(nowMilliseconds + 100, nextMilliseconds);
        frame(nowMilliseconds, unavailableViewerFrame("not-stereo"));
      }
    } else nowMilliseconds = nextMilliseconds;
    callback?.();
  };
  return { browser, documentRef, renderer, scene, click, launch, page, free, exportRecord, summary, sharedCommand, sharedView, pause, clearPauseReason, tick, advancePlayback, playbackClock, replayTicks, observeCountdown,
    countdownTick, countdown, acknowledgeWaterQuality,
    advanceClock: (elapsedMilliseconds: number) => { nowMilliseconds += elapsedMilliseconds; },
    sessionOwner: () => {
      const result = initializeSession.mock.results[0];
      if (result?.type !== "return") throw new Error("Main session owner is unavailable");
      return result.value;
    },
    endWebXr: () => {
      if (sessionEndHandler === null) throw new Error("WebXR session end handler is unavailable");
      sessionEndHandler();
    },
    runtimeDispose, releaseInitialization, initializationPaused: () => initializationPaused,
    frame: (timestamp: number) => { frame(timestamp, unavailableViewerFrame("not-stereo")); } };
}

function runFlightToResult(trial: Awaited<ReturnType<typeof fixture>>, initialTimestampMilliseconds: number,
  maximumElapsedMilliseconds: number): void {
  const frameMilliseconds = 100;
  for (let elapsed = frameMilliseconds; elapsed < maximumElapsedMilliseconds && trial.scene() === "Flight"; elapsed += frameMilliseconds) {
    trial.frame(initialTimestampMilliseconds + elapsed);
  }
  if (trial.scene() === "Flight") trial.frame(initialTimestampMilliseconds + maximumElapsedMilliseconds);
}

async function manualReplayFixture() {
  const trial = await fixture(false, false, false, true);
  await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
  trial.click("game-title-start");
  trial.click("game-setup-start");
  trial.launch();
  for (let timestamp = 0; timestamp <= 1000; timestamp += 100) trial.frame(timestamp);
  expect(trial.scene()).toBe("Flight");
  trial.click("game-flight-abort");
  trial.click("game-result-replay");
  trial.click("game-replay-play-pause");
  expect(trial.documentRef.querySelector('[data-control-id="game-replay-play-pause"]')?.textContent).toBe("Pause");
  expect(trial.replayTicks).toHaveLength(1);
  return {
    ...trial,
    replayTime: (): number => {
      const clock = trial.playbackClock.mock.results.at(-1);
      if (clock?.type !== "return") throw new Error("Rust Replay clock is missing");
      const time = clock.value[0];
      if (time === undefined || !Number.isFinite(time)) throw new Error("Rust Replay time is invalid");
      return time;
    },
    tickReplay: (callback = trial.replayTicks.shift()): void => {
      if (callback === undefined) throw new Error("Replay timer is missing");
      trial.advanceClock(50);
      callback();
    }
  };
}

describe("public main entrypoint with actual two-tail Rust WASM", () => {
  it("cancels the actual Rust Countdown on WebXR end delivery before UI phase synchronization", async () => {
    const trial = await fixture(false, false, false, false, "normal", true);
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("boot-enter-webxr");
    await vi.waitFor(() => { expect(trial.documentRef.querySelector('[data-control-id="boot-exit-vr"]')).not.toBeNull(); });
    const owner = await trial.sessionOwner();
    owner.executeSharedSessionCommand({ kind: "open_setup" });
    owner.executeSharedSessionCommand({ kind: "prepare" });
    owner.executeSharedSessionCommand({ kind: "start_countdown" });
    const obsolete = owner.captureCountdownToken();
    expect(owner.readLifecycle().phase).toBe("countdown");
    trial.endWebXr();
    await vi.waitFor(() => { expect(owner.readLifecycle().phase).toBe("briefing_ready"); });
    expect(owner.observeCountdown(obsolete, 3, true)).toEqual({ kind: "ignored" });
    expect(trial.observeCountdown.mock.results.filter((result) => result.type === "return" && result.value === 3)).toHaveLength(0);
    expect(trial.tick).not.toHaveBeenCalled();
  });

  it.each(["ui-update", "rejected-ui-update"] as const)("continues the same Countdown after query invalidation by %s", async (operation) => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    trial.click("game-setup-start");
    trial.click("game-briefing-start");
    const owner = await trial.sessionOwner();
    const query = owner.captureQueryToken();
    if (operation === "ui-update") {
      const update = owner.updateSharedUi({ kind: "session", command: { kind: "cancel_countdown" } }, sharedGameUiPlatform());
      expect(update.effects).toContainEqual({ kind: "session", command: { kind: "cancel_countdown" } });
    } else expect(() => owner.updateSharedUi({ kind: "toggle_technical" }, sharedGameUiPlatform())).toThrow();
    expect(owner.acceptQuery(query, undefined).kind).toBe("stale");
    expect(owner.readLifecycle().phase).toBe("countdown");
    trial.countdownTick(800);
    expect(trial.documentRef.querySelector(".screen-ui-shell")?.textContent).toContain("3 秒");
    trial.countdownTick(200);
    expect(trial.documentRef.querySelector(".screen-ui-shell")?.textContent).toContain("2 秒");
    trial.countdownTick(2000);
    expect(trial.scene()).toBe("Flight");
    expect(trial.observeCountdown.mock.results.filter((result) => result.type === "return" && result.value === 3)).toHaveLength(1);
  });

  it("uses monotonic elapsed instead of timer callback counts and rejects a duplicate callback", async () => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    trial.click("game-setup-start");
    trial.click("game-briefing-start");
    const duplicate = trial.countdown.shift();
    if (duplicate === undefined) throw new Error("Countdown observation is missing");
    trial.countdownTick(800, duplicate);
    expect(trial.scene()).toBe("Countdown");
    expect(trial.documentRef.querySelector(".screen-ui-shell")?.textContent).toContain("3 秒");
    duplicate();
    expect(trial.observeCountdown).toHaveBeenCalledTimes(1);
    trial.countdownTick(200);
    expect(trial.documentRef.querySelector(".screen-ui-shell")?.textContent).toContain("2 秒");
    const completed = trial.countdown.shift();
    trial.countdownTick(2000, completed);
    expect(trial.scene()).toBe("Flight");
    completed?.();
    expect(trial.observeCountdown.mock.results.filter((result) => result.type === "return" && result.value === 3)).toHaveLength(1);
    expect(trial.tick).not.toHaveBeenCalled();
  });

  it("rejects an old Countdown callback after cancel and restart", async () => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    trial.click("game-setup-start");
    trial.click("game-briefing-start");
    const obsolete = trial.countdown.shift();
    trial.click("game-countdown-cancel");
    trial.click("game-briefing-start");
    trial.countdownTick(800, obsolete);
    expect(trial.scene()).toBe("Countdown");
    expect(trial.documentRef.querySelector(".screen-ui-shell")?.textContent).toContain("3 秒");
    expect(trial.observeCountdown).not.toHaveBeenCalled();
    trial.countdownTick(200);
    expect(trial.documentRef.querySelector(".screen-ui-shell")?.textContent).toContain("2 秒");
  });

  it.each(["no-frames", "blur", "hidden", "late-frame"] as const)("cancels Countdown for %s without timer-driven launch", async (interruption) => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.frame(0);
    trial.click("game-title-start");
    trial.click("game-setup-start");
    trial.click("game-briefing-start");
    if (interruption === "blur") trial.browser.dispatchEvent(new trial.browser.Event("blur"));
    if (interruption === "hidden") {
      Object.defineProperty(trial.browser.document, "visibilityState", { value: "hidden", configurable: true });
      trial.browser.document.dispatchEvent(new trial.browser.Event("visibilitychange"));
    }
    if (interruption === "late-frame") {
      trial.advanceClock(151);
      trial.frame(151);
    }
    trial.countdownTick(3000, undefined, false);
    expect(trial.scene()).toBe("Briefing");
    expect(trial.observeCountdown.mock.results.filter((result) => result.type === "return" && result.value === 3)).toHaveLength(0);
    expect(trial.tick).not.toHaveBeenCalled();
  });
  it("projects ProcessingDelay, preserves ticks and requires explicit Resume without catch-up", async () => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    trial.click("game-setup-start");
    trial.launch();
    trial.frame(0);
    trial.frame(15);
    expect(trial.tick).toHaveBeenCalledTimes(1);
    trial.frame(1_015);
    expect(trial.tick).toHaveBeenCalledTimes(1);
    expect(trial.documentRef.querySelector('.screen-ui-shell')?.textContent).toContain("ProcessingDelay");
    const resume = (): HTMLButtonElement => {
      const button = trial.documentRef.querySelector<HTMLButtonElement>('button[data-control-id="game-flight-resume"]');
      if (button === null) throw new Error("ProcessingDelay must expose explicit Resume");
      return button;
    };
    expect(resume().disabled).toBe(true);
    trial.frame(2_015);
    expect(resume().disabled).toBe(true);
    trial.frame(2_025);
    expect(resume().disabled).toBe(false);
    expect(trial.tick).toHaveBeenCalledTimes(1);
    trial.click("game-flight-resume");
    trial.frame(10_000);
    trial.frame(10_005);
    expect(trial.tick).toHaveBeenCalledTimes(1);
    trial.frame(10_010);
    expect(trial.tick).toHaveBeenCalledTimes(2);
    trial.click("game-flight-abort");
    expect(trial.scene()).toBe("Result");
    expect(trial.exportRecord).toHaveReturned();
    const saved: unknown = JSON.parse(trial.exportRecord.mock.results.at(-1)?.value as string);
    expect(saved).toMatchObject({ samples: [{ tick_index: 0 }, { tick_index: 1 }, { tick_index: 2 }], finalization: { terminal_tick: 2 } });
  });
  it.each([
    ["loading", "保存記録を読み込んでいる"],
    ["failed", "FlightRecord一覧を取得できない: Injected record listing failure"],
    ["normal", "保存済みFlightRecordはない"]
  ] as const)("projects %s record-list feedback into the Rust Title view without leaking into other scenes", async (mode, message) => {
    const trial = await fixture(false, false, false, false, mode);
    const observedView = () => {
      const result = trial.sharedView.mock.results.at(-1);
      if (result?.type !== "return" || typeof result.value !== "string") throw new Error("Shared Rust UI view is unavailable");
      return readContract<GameUiView>("view", result.value);
    };
    await vi.waitFor(() => {
      expect(trial.scene()).toBe("Title");
      expect(trial.documentRef.querySelector(".screen-ui-shell")?.textContent).toContain(message);
    });
    const titleView = observedView();
    expect(titleView.description).toContain(message);
    const titleStatus = titleView.panels.flatMap((panel) => panel.controls).find((control) => control.id === "game-state")?.content;
    if (titleStatus?.kind !== "status") throw new Error("Shared Title status is unavailable");
    expect(titleStatus.value).toContain(message);
    const assertNoListFeedback = () => {
      expect(JSON.stringify(observedView())).not.toContain(message);
      expect(trial.documentRef.querySelector(".screen-ui-shell")?.textContent).not.toContain(message);
    };
    trial.click("game-title-start");
    expect(trial.scene()).toBe("FlightSetup");
    assertNoListFeedback();
    trial.click("game-setup-start");
    expect(trial.scene()).toBe("Briefing");
    assertNoListFeedback();
    trial.click("game-briefing-start");
    expect(trial.scene()).toBe("Countdown");
    assertNoListFeedback();
    for (let tick = 0; tick < 3; tick += 1) trial.countdownTick();
    expect(trial.scene()).toBe("Flight");
    trial.frame(0);
    trial.frame(10);
    assertNoListFeedback();
    trial.click("game-flight-pause");
    expect(observedView().active_overlay).toBe("Pause");
    assertNoListFeedback();
    trial.click("game-flight-resume");
    trial.click("game-flight-abort");
    expect(trial.scene()).toBe("Result");
    assertNoListFeedback();
    trial.click("game-result-replay");
    expect(trial.scene()).toBe("Replay");
    assertNoListFeedback();
    trial.click("game-replay-return");
    trial.click("game-result-title");
    expect(trial.scene()).toBe("Title");
    if (mode !== "normal") await vi.waitFor(() => { expect(observedView().description).toContain(message); });
    else await vi.waitFor(() => {
      expect(observedView().panels.flatMap((panel) => panel.controls).some((control) => control.id === "game-title-open-record-1")).toBe(true);
      expect(trial.documentRef.querySelector('[data-control-id="game-title-open-record-1"]')).not.toBeNull();
      expect(observedView().description).not.toContain(message);
    });
    trial.click("game-title-demo");
    expect(trial.documentRef.querySelector('[data-control-id="game-attract-return"]')).not.toBeNull();
    assertNoListFeedback();
  });

  it("continues the same Rust Replay clock after display and camera changes", async () => {
    const trial = await manualReplayFixture();
    trial.tickReplay();
    let previousTime = trial.replayTime();
    expect(previousTime).toBeGreaterThan(0);
    for (const controlId of ["game-replay-view-mode", "game-replay-camera", "game-replay-view-mode", "game-replay-view-mode"]) {
      const previousLabel = trial.documentRef.querySelector(`[data-control-id="${controlId}"]`)?.textContent;
      trial.click(controlId);
      expect(trial.documentRef.querySelector(`[data-control-id="${controlId}"]`)?.textContent).not.toBe(previousLabel);
      trial.tickReplay();
      const time = trial.replayTime();
      expect(time).toBeGreaterThan(previousTime);
      previousTime = time;
      expect(trial.replayTicks).toHaveLength(1);
      expect(trial.documentRef.querySelector('[data-control-id="game-replay-play-pause"]')?.textContent).toBe("Pause");
    }
    expect(trial.advancePlayback).toHaveBeenCalledTimes(5);
    expect(trial.free).not.toHaveBeenCalled();
  });

  it.each(["pause", "return", "retry", "pagehide"] as const)("rejects the obsolete Replay timer after %s", async (operation) => {
    const trial = await manualReplayFixture();
    const obsoleteTick = trial.replayTicks.shift();
    if (obsoleteTick === undefined) throw new Error("Replay timer is missing");
    if (operation === "pagehide") {
      trial.page("pagehide", false);
      expect(trial.free).toHaveBeenCalledTimes(1);
    } else if (operation === "pause") {
      trial.click("game-replay-play-pause");
      trial.click("game-replay-play-pause");
    } else {
      trial.click("game-replay-return");
      if (operation === "retry") {
        trial.click("game-result-retry");
        trial.launch();
        trial.frame(2000);
        trial.frame(2100);
        trial.frame(2200);
        trial.click("game-flight-abort");
      }
      trial.click("game-result-replay");
      trial.click("game-replay-play-pause");
    }
    trial.tickReplay(obsoleteTick);
    expect(trial.advancePlayback).not.toHaveBeenCalled();
    expect(trial.replayTicks).toHaveLength(operation === "pagehide" ? 0 : 1);
    if (operation !== "pagehide") {
      trial.tickReplay();
      expect(trial.advancePlayback).toHaveBeenCalledTimes(1);
      expect(trial.replayTime()).toBeGreaterThan(0);
      expect(trial.replayTicks).toHaveLength(1);
    }
  });

  it("waits for quality application and retains the selection through weather changes and Retry", async () => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    let complete: () => void = () => { throw new Error("Quality request is missing"); };
    trial.renderer.setLakeWaterQuality.mockImplementationOnce(() => new Promise<LakeWaterQualityCleanupResult>((resolve) => {
      complete = () => { trial.acknowledgeWaterQuality("low"); resolve({ kind: "complete" }); };
    }));
    trial.click("presentation-water-quality-low");
    expect(trial.renderer.setLakeWaterQuality).toHaveBeenCalledWith("low");
    expect(trial.documentRef.querySelector("output#presentation-water-quality-status")?.textContent).toContain("Lowを適用中");
    expect(trial.documentRef.querySelector('[data-control-id="presentation-water-quality-high"]')?.getAttribute("aria-pressed")).toBe("true");
    complete();
    await vi.waitFor(() => { expect(trial.documentRef.querySelector('[data-control-id="presentation-water-quality-low"]')?.getAttribute("aria-pressed")).toBe("true"); });
    trial.click("game-setup-select-weather-2");
    expect(trial.documentRef.querySelector('[data-control-id="presentation-water-quality-low"]')?.getAttribute("aria-pressed")).toBe("true");
    trial.click("game-setup-start");
    trial.launch();
    trial.frame(0); trial.frame(10);
    trial.click("game-flight-abort");
    trial.click("game-result-retry");
    trial.launch();
    trial.frame(20); trial.frame(30);
    trial.click("game-flight-pause");
    trial.click("game-pause-open-settings");
    expect(trial.documentRef.querySelector('[data-control-id="presentation-water-quality-low"]')?.getAttribute("aria-pressed")).toBe("true");
    expect(trial.renderer.setLakeWaterQuality).toHaveBeenCalledTimes(1);
  });

  it("shows a quality failure without changing the Rust setup selection and permits retry", async () => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    const weather = trial.documentRef.querySelector("output#game-setup-weather-current")?.textContent;
    expect(weather).toBeTruthy();
    trial.renderer.setLakeWaterQuality.mockRejectedValueOnce(new Error("Injected quality allocation failure"));
    trial.click("presentation-water-quality-medium");
    await vi.waitFor(() => { expect(trial.documentRef.querySelector("output#presentation-water-quality-status")?.textContent).toContain("Injected quality allocation failure"); });
    expect(trial.documentRef.querySelector('[data-control-id="presentation-water-quality-high"]')?.getAttribute("aria-pressed")).toBe("true");
    expect(trial.documentRef.querySelector("output#game-setup-weather-current")?.textContent).toBe(weather);
    trial.click("presentation-water-quality-medium");
    await vi.waitFor(() => { expect(trial.documentRef.querySelector('[data-control-id="presentation-water-quality-medium"]')?.getAttribute("aria-pressed")).toBe("true"); });
    expect(trial.renderer.setLakeWaterQuality).toHaveBeenCalledTimes(2);
  });

  it("reflects an applied quality despite a reported post-swap cleanup failure", async () => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    trial.renderer.setLakeWaterQuality.mockImplementationOnce((quality) => {
      trial.acknowledgeWaterQuality(quality);
      return Promise.resolve({ kind: "failed", message: "Injected retired geometry cleanup failure" });
    });
    trial.click("presentation-water-quality-medium");
    await vi.waitFor(() => { expect(trial.documentRef.querySelector('[data-control-id="presentation-water-quality-medium"]')?.getAttribute("aria-pressed")).toBe("true"); });
    expect(trial.documentRef.querySelector("output#presentation-water-quality-status")?.textContent).toContain("Mediumを適用した。旧geometryの解放に失敗した");
  });

  it("uses selected and sealed sky through Retry and Replay and resolves the recorded current environment", async () => {
    const trial = await fixture(false, true);
    await vi.waitFor(() => { expect(trial.documentRef.querySelector('[data-control-id="game-title-open-record-1"]')).not.toBeNull(); });
    expect(trial.renderer.setLakeSkyCondition.mock.lastCall?.[0]).toBeNull();
    trial.click("game-title-start");
    trial.click("game-setup-select-weather-2");
    const condition = { sunAzimuthDegrees: 135, sunElevationDegrees: 55, cloudFraction: 0.25, cloudBaseMeters: 1500, visibilityMeters: 25_000 };
    expect(trial.renderer.setLakeSkyCondition.mock.lastCall?.[0]).toEqual(condition);
    trial.click("game-setup-start");
    expect(trial.renderer.setLakeSkyCondition.mock.lastCall?.[0]).toEqual(condition);
    trial.launch();
    trial.frame(0);
    trial.frame(10);
    trial.click("game-flight-abort");
    expect(trial.renderer.setLakeSkyCondition.mock.lastCall?.[0]).toEqual(condition);
    trial.click("game-result-retry");
    expect(trial.renderer.setLakeSkyCondition.mock.lastCall?.[0]).toEqual(condition);
    trial.launch();
    trial.click("game-flight-abort");
    trial.click("game-result-replay");
    expect(trial.renderer.setLakeSkyCondition.mock.lastCall?.[0]).toEqual(condition);
    trial.click("game-replay-return");
    trial.click("game-result-title");
    trial.click("game-title-open-record-1");
    await vi.waitFor(() => { expect(trial.scene()).toBe("Replay"); });
    expect(trial.renderer.setLakeSkyCondition.mock.lastCall?.[0]).toEqual(condition);
    expect(trial.renderer.setFlightPose.mock.lastCall?.[0]?.controls?.layout).toBe("tail_incidence");
  });

  it("projects shared Screen scale choices through the real WASM UI and retains the preference through flight and Retry", async () => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    expect(trial.renderer.setScreenRenderScale).not.toHaveBeenCalled();
    trial.click("game-title-start");
    for (const [scale, factor] of [["75", 0.75], ["50", 0.5], ["100", 1]] as const) {
      trial.click(`presentation-render-scale-${scale}`);
      expect(trial.renderer.setScreenRenderScale.mock.lastCall).toEqual([factor]);
      expect(trial.documentRef.querySelector(`[data-control-id="presentation-render-scale-${scale}"]`)?.getAttribute("aria-pressed")).toBe("true");
      expect(trial.tick).not.toHaveBeenCalled();
    }
    trial.click("presentation-render-scale-auto");
    expect(trial.renderer.setScreenRenderAuto.mock.lastCall).toEqual([true]);
    expect(trial.documentRef.querySelector('[data-control-id="presentation-render-scale-auto"]')?.getAttribute("aria-pressed")).toBe("true");
    trial.click("presentation-render-scale-100");
    expect(trial.documentRef.querySelector('[data-control-id="presentation-render-scale-auto"]')?.getAttribute("aria-pressed")).toBe("false");
    expect(trial.tick).not.toHaveBeenCalled();
    trial.click("game-setup-start");
    trial.launch();
    trial.frame(0);
    trial.frame(10);
    trial.click("game-flight-pause");
    trial.click("game-pause-open-settings");
    const before = trial.tick.mock.calls.length;
    const back = trial.documentRef.querySelector<HTMLButtonElement>('[data-control-id="game-pause-settings-back"]');
    back?.focus();
    trial.click("presentation-render-scale-50");
    expect(trial.renderer.setScreenRenderScale.mock.lastCall).toEqual([0.5]);
    expect(trial.tick.mock.calls).toHaveLength(before);
    expect(trial.documentRef.querySelector('[data-control-id="game-pause-settings-back"]')).toBe(back);
    expect(trial.documentRef.activeElement).toBe(back);
    trial.click("game-pause-settings-back");
    trial.click("game-flight-resume");
    trial.click("game-flight-abort");
    trial.click("game-result-retry");
    expect(trial.scene()).toBe("Briefing");
    expect(trial.renderer.setScreenRenderScale.mock.lastCall).toEqual([0.5]);
  });

  it("projects the sealed aircraft throughout Briefing and Countdown and replaces it with the identical first live pose", async () => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    expect(trial.renderer.setPreparedFlightPose.mock.lastCall?.[0]).toBeNull();
    trial.click("game-title-start");
    trial.click("game-setup-start");
    expect(trial.scene()).toBe("Briefing");
    const initial = trial.renderer.setPreparedFlightPose.mock.lastCall?.[0];
    if (initial === null || initial === undefined) throw new Error("Missing Rust sealed launch pose");
    expect(initial).toMatchObject({ controls: { layout: "tail_incidence" }, simulationTimeSeconds: 0 });
    trial.click("game-briefing-start");
    for (let step = 0; step < 3; step++) {
      expect(trial.scene()).toBe("Countdown");
      trial.frame(step * 10);
      expect(trial.renderer.setPreparedFlightPose.mock.lastCall?.[0]).toEqual(initial);
      expect(trial.tick).not.toHaveBeenCalled();
      trial.countdownTick();
    }
    expect(trial.scene()).toBe("Flight");
    expect(trial.renderer.setPreparedFlightPose.mock.lastCall?.[0]).toBeNull();
    expect(trial.renderer.setFlightPose.mock.lastCall?.[0]).toMatchObject(initial);
    trial.click("game-flight-abort");
    trial.click("game-result-retry");
    expect(trial.scene()).toBe("Briefing");
    expect(trial.renderer.setPreparedFlightPose.mock.lastCall?.[0]).toEqual(initial);
    trial.click("game-briefing-cancel");
    expect(trial.scene()).toBe("FlightSetup");
    expect(trial.renderer.setPreparedFlightPose.mock.lastCall?.[0]).toBeNull();
  });
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

  it.each(["running", "paused"] as const)("keeps the %s shared Abort Result inert across delayed frames", async (phase) => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    trial.click("game-setup-start");
    trial.launch();
    trial.frame(0);
    trial.frame(10);
    if (phase === "paused") trial.click("game-flight-pause");
    trial.click(phase === "paused" ? "game-paused-abort" : "game-flight-abort");
    expect(trial.scene()).toBe("Result");
    const ticks = trial.tick.mock.calls.length;
    await vi.waitFor(() => { expect(trial.exportRecord).toHaveBeenCalledTimes(1); });
    const record = trial.exportRecord.mock.results[0]?.value as string;
    expect(JSON.parse(record)).toMatchObject({ schema_version: 6, finalization: { reason: "manual_abort" } });
    trial.frame(1_000);
    trial.frame(1_010);
    trial.frame(5_000);
    expect(trial.pause).not.toHaveBeenCalled();
    expect(trial.clearPauseReason).not.toHaveBeenCalled();
    expect(trial.tick).toHaveBeenCalledTimes(ticks);
    expect(trial.exportRecord).toHaveBeenCalledTimes(1);
    expect(trial.browser.document.body.textContent).not.toContain("InvalidTransition");
    expect(trial.sharedCommand.mock.calls.filter(([json]) => {
      const command: unknown = JSON.parse(json);
      return typeof command === "object" && command !== null && "kind" in command && command.kind === "abort";
    })).toHaveLength(1);
  });

  it("retains a manual Result and its original snapshot-rendering failure without replaying domain commands", async () => {
    const trial = await fixture();
    await vi.waitFor(() => { expect(trial.scene()).toBe("Title"); });
    trial.click("game-title-start");
    trial.click("game-setup-start");
    trial.launch();
    trial.frame(0);
    trial.frame(10);
    trial.renderer.setFlightPose.mockImplementationOnce(() => { throw new Error("Injected terminal snapshot rendering failure"); });
    trial.click("game-flight-abort");
    expect(trial.scene()).toBe("Result");
    expect(trial.browser.document.body.textContent).toContain("Injected terminal snapshot rendering failure");
    expect(trial.exportRecord).toHaveBeenCalledTimes(1);
    expect(trial.summary).toHaveBeenCalled();
    expect(JSON.parse(trial.exportRecord.mock.results[0]?.value as string)).toMatchObject({ finalization: { reason: "manual_abort" } });
    trial.frame(1_000);
    expect(trial.pause).not.toHaveBeenCalled();
    expect(trial.clearPauseReason).not.toHaveBeenCalled();
    expect(trial.exportRecord).toHaveBeenCalledTimes(1);
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

  it("opens a current saved snapshot through the Tail owner with its registered venue and two-tail controls", async () => {
    const trial = await fixture(false, true);
    await vi.waitFor(() => { expect(trial.documentRef.querySelector('[data-control-id="game-title-open-record-1"]')).not.toBeNull(); });
    expect(trial.renderer.setLakeVenueVisible.mock.lastCall?.[0]).toBe(true);
    trial.click("game-title-open-record-1");
    await vi.waitFor(() => { expect(trial.scene()).toBe("Replay"); });
    expect(trial.renderer.setLakeVenueVisible.mock.lastCall?.[0]).toBe(true);
    expect(trial.renderer.setFlightPose.mock.calls.at(-1)?.[0]?.controls?.layout).toBe("tail_incidence");
    expect(trial.exportRecord).not.toHaveBeenCalled();
    trial.click("game-replay-view-mode");
    trial.click("game-replay-speed-2");
    trial.click("game-replay-return");
    expect(trial.scene()).toBe("Title");
    expect(trial.renderer.setLakeVenueVisible.mock.lastCall?.[0]).toBe(true);
    trial.click("game-title-start");
    expect(trial.renderer.setLakeVenueVisible.mock.lastCall?.[0]).toBe(true);
    trial.click("game-setup-start");
    expect(trial.renderer.setLakeVenueVisible.mock.lastCall?.[0]).toBe(true);
    trial.launch();
    expect(trial.renderer.setLakeVenueVisible.mock.lastCall?.[0]).toBe(true);
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
      const terminal = reason === "manual_abort" ? trial.sharedCommand.mock.calls.some(([json], index) => {
        const command = JSON.parse(json) as { readonly kind: string };
        return command.kind === "abort" && trial.sharedCommand.mock.results[index]?.type === "return";
      })
        : trial.tick.mock.results.some((result) => {
          const snapshot: unknown = JSON.parse(result.value as string);
          return typeof snapshot === "object" && snapshot !== null && "phase" in snapshot && snapshot.phase === "result";
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
