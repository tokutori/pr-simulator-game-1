import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { RendererAdapter, StereoPresentationProfile } from "../../web/src/render/contracts/runtime.js";
import type { UiActionDispatcher } from "../../web/src/render/contracts/ui.js";
import { createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { PhoneVrPresentationBackend } from "../../web/src/presentation/phone-vr-backend.js";
import type { PhoneVrGamepadState, PhoneVrPermissionResult, PhoneVrSensorPort, PhoneVrSensorReading } from "../../web/src/presentation/phone-vr-contracts.js";
import { PHONE_VR_OPTICAL_PROFILE } from "../../web/src/presentation/phone-vr-contracts.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { ScreenPresentationBackend } from "../../web/src/presentation/screen-backend.js";

type SensorCallbacks = {
  readonly reading: (reading: PhoneVrSensorReading) => void;
  readonly rotation: (angle: number | null) => void;
};

const view = createSceneFixture("Title");
const viewport = () => ({ x: 1280, y: 720, pixelRatio: 1 });
const valid = (timestampMs = 100, beta = 90): PhoneVrSensorReading => ({
  alpha: 0, beta, gamma: 0, timestampMs
});
const backends: PhoneVrPresentationBackend[] = [];

beforeEach(() => { vi.useFakeTimers(); });
afterEach(async () => {
  for (const backend of backends.splice(0)) await backend.stop().catch(() => undefined);
  vi.clearAllTimers();
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("Phone VR startup attempt ownership", () => {
  const invalidNotifications = [
    { label: "null angle", send: (callbacks: SensorCallbacks) => { callbacks.reading({ ...valid(), gamma: null }); }, message: "null or non-finite" },
    { label: "non-finite timestamp", send: (callbacks: SensorCallbacks) => { callbacks.reading(valid(Number.NaN)); }, message: "timestamp is invalid" },
    { label: "future timestamp", send: (callbacks: SensorCallbacks) => { callbacks.reading(valid(201)); }, message: "timestamp is invalid" },
    { label: "backward timestamp", send: (callbacks: SensorCallbacks) => { callbacks.reading(valid(99)); }, message: "timestamp is invalid" },
    { label: "missing screen orientation", send: (callbacks: SensorCallbacks) => { callbacks.rotation(null); }, message: "screen orientation became unavailable" },
    { label: "non-finite screen orientation", send: (callbacks: SensorCallbacks) => { callbacks.rotation(Number.NaN); }, message: "screen orientation became unavailable" }
  ] as const;

  it.each(invalidNotifications)("retains the synchronous first failure after a valid sample: $label", async ({ send, message }) => {
    const fixture = createFixture();
    fixture.sensors.onStart = (callbacks) => {
      callbacks.reading(valid());
      send(callbacks);
      callbacks.reading(valid(101, 60));
    };
    await fixture.backend.requestPermissionFromUserGesture();
    await expect(fixture.backend.start()).rejects.toThrow(message);
    expect(fixture.backend.currentFrame(200, view).panelVisible).toBe(false);
    expect(fixture.unavailable).not.toHaveBeenCalled();
    expectReleased(fixture);
  });

  it("accepts multiple synchronous valid samples and retains the latest pose", async () => {
    const fixture = createFixture();
    fixture.sensors.onStart = (callbacks) => {
      callbacks.reading(valid());
      callbacks.reading(valid(101, 70));
    };
    await fixture.backend.requestPermissionFromUserGesture();
    await fixture.backend.start();
    expect(fixture.backend.currentFrame(200, view).panelVisible).toBe(true);
    expect(fixture.backend.currentFrame(200, view).cameraPose.orientation.x).not.toBe(0);
    expect(fixture.stereo()).toEqual(PHONE_VR_OPTICAL_PROFILE);
    expect(vi.getTimerCount()).toBe(0);
  });

  it.each(["stop", "cancel"] as const)("settles startup before its initial sample on direct %s", async (operation) => {
    const fixture = createFixture();
    await fixture.backend.requestPermissionFromUserGesture();
    const observed = observe(fixture.backend.start());
    await end(fixture.backend, operation);
    await flushMicrotasks();
    expectRejected(observed.result());
    expectReleased(fixture);
    await expect(fixture.backend.start()).rejects.toThrow("explicit user permission");
  });

  it.each(["stop", "cancel"] as const)("rejects a resolved initial sample when %s occurs before the startup continuation", async (operation) => {
    const fixture = createFixture();
    await fixture.backend.requestPermissionFromUserGesture();
    const observed = observe(fixture.backend.start());
    fixture.sensors.current().reading(valid());
    await end(fixture.backend, operation);
    await flushMicrotasks();
    expectRejected(observed.result());
    expectReleased(fixture);
  });

  it.each(["stop", "cancel"] as const)("ignores callbacks from the previous %s attempt during retry", async (operation) => {
    const fixture = createFixture();
    await fixture.backend.requestPermissionFromUserGesture();
    const first = observe(fixture.backend.start());
    const previousCallbacks = fixture.sensors.current();
    await end(fixture.backend, operation);
    await flushMicrotasks();
    expect(first.result().type).toBe("rejected");
    expect(await fixture.backend.requestPermissionFromUserGesture()).toEqual({ ok: true });
    const second = observe(fixture.backend.start());
    const currentCallbacks = fixture.sensors.current();
    previousCallbacks.reading(valid());
    previousCallbacks.reading({ ...valid(), gamma: null });
    previousCallbacks.rotation(null);
    await flushMicrotasks();
    expect(second.result().type).toBe("pending");
    expect(fixture.clock).not.toHaveBeenCalled();
    expect(fixture.sensors.current()).toBe(currentCallbacks);
    expect(vi.getTimerCount()).toBe(1);
    currentCallbacks.reading(valid());
    await flushMicrotasks();
    expect(second.result().type).toBe("resolved");
    const currentFrame = fixture.backend.currentFrame(200, view);
    previousCallbacks.reading(valid(101, 40));
    previousCallbacks.rotation(90);
    previousCallbacks.reading({ ...valid(), gamma: null });
    expect(fixture.backend.currentFrame(200, view)).toEqual(currentFrame);
    expect(fixture.stereo()).toEqual(PHONE_VR_OPTICAL_PROFILE);
    expect(fixture.unavailable).not.toHaveBeenCalled();
    expect(fixture.sensors.stopListening).toHaveBeenCalledTimes(1);
  });

  it("keeps old startup continuations from releasing a new attempt", async () => {
    const fixture = createFixture();
    await fixture.backend.requestPermissionFromUserGesture();
    const first = observe(fixture.backend.start());
    const stopped = fixture.backend.stop();
    const permission = fixture.backend.requestPermissionFromUserGesture();
    expect(await permission).toEqual({ ok: true });
    const second = observe(fixture.backend.start());
    await stopped;
    await flushMicrotasks();
    expect(first.result().type).toBe("rejected");
    expect(second.result().type).toBe("pending");
    expect(vi.getTimerCount()).toBe(1);
    expect(fixture.stereo()).toEqual(PHONE_VR_OPTICAL_PROFILE);
    fixture.sensors.current().reading(valid());
    await flushMicrotasks();
    expect(second.result().type).toBe("resolved");
    expect(fixture.sensors.stopListening).toHaveBeenCalledTimes(1);
  });

  it("times out exactly once and accepts a fresh permission and sample on retry", async () => {
    const fixture = createFixture();
    await fixture.backend.requestPermissionFromUserGesture();
    const observed = observe(fixture.backend.start());
    await vi.advanceTimersByTimeAsync(49);
    expect(observed.result().type).toBe("pending");
    await vi.advanceTimersByTimeAsync(1);
    expect(expectRejected(observed.result()).message).toContain("before timeout");
    expectReleased(fixture);
    await fixture.backend.requestPermissionFromUserGesture();
    const startup = fixture.backend.start();
    fixture.sensors.current().reading(valid());
    await startup;
    await vi.advanceTimersByTimeAsync(100);
    expect(fixture.backend.currentFrame(200, view).panelVisible).toBe(true);
    expect(fixture.sensors.stopListening).toHaveBeenCalledTimes(1);
  });

  it("ignores a captured timeout from a released attempt while a new sample wait is pending", async () => {
    const timers = vi.spyOn(globalThis, "setTimeout");
    const fixture = createFixture();
    await fixture.backend.requestPermissionFromUserGesture();
    const first = observe(fixture.backend.start());
    const oldTimeout = timers.mock.calls[0]?.[0];
    if (typeof oldTimeout !== "function") throw new Error("Missing first-sample timeout callback");
    await fixture.backend.cancelPendingRequest();
    await flushMicrotasks();
    expect(first.result().type).toBe("rejected");
    await fixture.backend.requestPermissionFromUserGesture();
    const second = observe(fixture.backend.start());
    Reflect.apply(oldTimeout, undefined, []);
    await flushMicrotasks();
    expect(second.result().type).toBe("pending");
    expect(vi.getTimerCount()).toBe(1);
    fixture.sensors.current().reading(valid());
    await flushMicrotasks();
    expect(second.result().type).toBe("resolved");
    expect(fixture.sensors.stopListening).toHaveBeenCalledTimes(1);
  });

  it.each(["orientation", "stereo", "listening"] as const)("terminates startup on a synchronous %s setup exception", async (operation) => {
    const fixture = createFixture();
    const failure = new Error(operation + " setup failed");
    if (operation === "orientation") fixture.sensors.getScreenOrientationAngle.mockImplementationOnce(() => { throw failure; });
    if (operation === "stereo") {
      const applyStereo = fixture.renderer.setStereoPresentation.getMockImplementation();
      fixture.renderer.setStereoPresentation.mockImplementationOnce((profile) => { applyStereo?.(profile); throw failure; });
    }
    if (operation === "listening") fixture.sensors.onStart = () => { throw failure; };
    await fixture.backend.requestPermissionFromUserGesture();
    await expect(fixture.backend.start()).rejects.toBe(failure);
    expectReleased(fixture);
    fixture.sensors.onStart = () => undefined;
    expect(await fixture.backend.requestPermissionFromUserGesture()).toEqual({ ok: true });
    const startup = fixture.backend.start();
    fixture.sensors.current().reading(valid());
    await startup;
  });

  it.each([false, true])("keeps the first sensor failure when setup throws afterward without an unobserved rejection (initial valid: %s)", async (initialValid) => {
    const fixture = createFixture();
    fixture.sensors.onStart = (callbacks) => {
      if (initialValid) callbacks.reading(valid());
      callbacks.reading({ ...valid(), gamma: null });
      throw new Error("later listener setup error");
    };
    await fixture.backend.requestPermissionFromUserGesture();
    await expect(fixture.backend.start()).rejects.toThrow("null or non-finite");
    await flushMicrotasks();
    expectReleased(fixture);
  });

  it.each(["selection", "listening", "stereo"] as const)("preserves startup failure and attempts all cleanup when %s release throws", async (operation) => {
    const fixture = createFixture();
    const cleanupFailure = new Error(operation + " release failed");
    if (operation === "selection") fixture.renderer.setSelectRayHandler.mockImplementationOnce(() => { throw cleanupFailure; });
    if (operation === "listening") fixture.sensors.stopListening.mockImplementationOnce(() => { fixture.sensors.detach(); throw cleanupFailure; });
    if (operation === "stereo") fixture.renderer.setStereoPresentation.mockImplementationOnce(() => undefined).mockImplementationOnce(() => { throw cleanupFailure; });
    fixture.sensors.onStart = (callbacks) => { callbacks.reading({ ...valid(), gamma: null }); };
    await fixture.backend.requestPermissionFromUserGesture();
    await expect(fixture.backend.start()).rejects.toThrow("null or non-finite");
    expect(fixture.sensors.stopListening).toHaveBeenCalledTimes(1);
    expect(fixture.renderer.setStereoPresentation).toHaveBeenLastCalledWith(null);
    expect(fixture.renderer.setSelectRayHandler).toHaveBeenCalledWith(null);
    expect(vi.getTimerCount()).toBe(0);
    expect(fixture.backend.currentFrame(200, view).panelVisible).toBe(false);
  });

  it.each(["stop", "cancel"] as const)("settles startup before a throwing cleanup on %s", async (operation) => {
    const fixture = createFixture();
    await fixture.backend.requestPermissionFromUserGesture();
    const observed = observe(fixture.backend.start());
    fixture.renderer.setSelectRayHandler.mockImplementationOnce(() => { throw new Error("selection release failed"); });
    await expect(end(fixture.backend, operation)).rejects.toThrow("selection release failed");
    await flushMicrotasks();
    expectRejected(observed.result());
    expectReleased(fixture);
  });

  it("keeps the original failure when cancellation precedes its continuation", async () => {
    const fixture = createFixture();
    await fixture.backend.requestPermissionFromUserGesture();
    const startup = fixture.backend.start();
    fixture.sensors.current().reading({ ...valid(), gamma: null });
    await fixture.backend.cancelPendingRequest();
    await expect(startup).rejects.toThrow("null or non-finite");
    expectReleased(fixture);
  });

  it.each([
    { input: "gaze", operation: "stop" },
    { input: "gaze", operation: "tracking failure" },
    { input: "gamepad", operation: "stop" },
    { input: "gamepad", operation: "tracking failure" }
  ] as const)("terminates $operation despite a focused $input dispatcher throwing and rejecting reentrant startup", async ({ input, operation }) => {
    const fixture = createFixture();
    if (input === "gamepad") fixture.gamepad.mockReturnValue({ axes: [], buttons: [] });
    await fixture.backend.requestPermissionFromUserGesture();
    const startup = fixture.backend.start();
    const oldCallbacks = fixture.sensors.current();
    oldCallbacks.reading(valid());
    await startup;
    fixture.backend.currentFrame(200, view);
    expect(fixture.dispatch).toHaveBeenCalledWith({ type: "focus", controlId: "title-toggle" });
    const reentrantPermission: Promise<PhoneVrPermissionResult>[] = [];
    const reentrantStartup: ReturnType<typeof observe>[] = [];
    const focusFailure = new Error("focus release failed");
    fixture.dispatch.mockImplementation((action) => {
      if (action.type !== "focus" || action.controlId !== null) return;
      reentrantPermission.push(fixture.backend.requestPermissionFromUserGesture());
      reentrantStartup.push(observe(fixture.backend.start()));
      oldCallbacks.reading(valid(101, 40));
      throw focusFailure;
    });
    if (operation === "stop") {
      await expect(fixture.backend.stop()).rejects.toBe(focusFailure);
      expect(fixture.unavailable).not.toHaveBeenCalled();
    } else {
      expect(() => { oldCallbacks.reading({ ...valid(), gamma: null }); }).not.toThrow();
      expect(fixture.unavailable).toHaveBeenCalledExactlyOnceWith("Phone VR orientation data contains null or non-finite values");
    }
    expect(reentrantPermission).toHaveLength(1);
    expect(await reentrantPermission[0]).toEqual({ ok: false, message: "A Phone VR sensor session is already active or stopping" });
    await flushMicrotasks();
    expect(reentrantStartup).toHaveLength(1);
    const attemptedStartup = reentrantStartup[0];
    if (attemptedStartup === undefined) throw new Error("Missing reentrant startup observation");
    expect(expectRejected(attemptedStartup.result()).message).toContain("explicit user permission");
    expectReleased(fixture);
    expect(fixture.backend.currentFrame(200, view).panelVisible).toBe(false);
    expect(await fixture.backend.requestPermissionFromUserGesture()).toEqual({ ok: true });
    const canceledRetry = observe(fixture.backend.start());
    await fixture.backend.cancelPendingRequest();
    await flushMicrotasks();
    expect(expectRejected(canceledRetry.result()).message).toContain("canceled");
    expect(reentrantPermission).toHaveLength(1);
    await fixture.backend.requestPermissionFromUserGesture();
    const failedRetry = fixture.backend.start();
    fixture.sensors.current().reading({ ...valid(), gamma: null });
    await expect(failedRetry).rejects.toThrow("null or non-finite");
    expect(reentrantPermission).toHaveLength(1);
    fixture.dispatch.mockReset();
    await fixture.backend.requestPermissionFromUserGesture();
    const validRetry = fixture.backend.start();
    fixture.sensors.current().reading(valid());
    await validRetry;
    fixture.backend.currentFrame(200, view);
    expect(fixture.dispatch).toHaveBeenCalledExactlyOnceWith({ type: "focus", controlId: "title-toggle" });
  });

  it("attempts both focused selector releases and all resources while preserving the first cleanup exception", async () => {
    const fixture = createFixture();
    await fixture.backend.requestPermissionFromUserGesture();
    const startup = fixture.backend.start();
    fixture.sensors.current().reading(valid());
    await startup;
    fixture.backend.currentFrame(200, view);
    const focusFailure = new Error("focus dispatch failed");
    fixture.dispatch.mockImplementation((action) => {
      if (action.type === "focus" && action.controlId === null) throw focusFailure;
    });
    fixture.gamepad.mockReturnValue({ axes: [], buttons: [] });
    expect(() => fixture.backend.currentFrame(201, view)).toThrow(focusFailure);
    fixture.dispatch.mockClear();
    const firstFailure = new Error("select handler cleanup failed");
    fixture.renderer.setSelectRayHandler.mockImplementationOnce(() => { throw firstFailure; });
    fixture.sensors.stopListening.mockImplementationOnce(() => { fixture.sensors.detach(); throw new Error("listener cleanup failed"); });
    fixture.renderer.setStereoPresentation.mockImplementationOnce(() => { throw new Error("stereo cleanup failed"); });
    await expect(fixture.backend.stop()).rejects.toBe(firstFailure);
    expect(fixture.dispatch).toHaveBeenCalledTimes(2);
    expect(fixture.dispatch).toHaveBeenNthCalledWith(1, { type: "focus", controlId: null });
    expect(fixture.dispatch).toHaveBeenNthCalledWith(2, { type: "focus", controlId: null });
    expect(fixture.sensors.stopListening).toHaveBeenCalledTimes(1);
    expect(fixture.renderer.setStereoPresentation).toHaveBeenLastCalledWith(null);
    expect(vi.getTimerCount()).toBe(0);
    expect(await fixture.backend.requestPermissionFromUserGesture()).toEqual({ ok: true });
    const retry = fixture.backend.start();
    fixture.sensors.current().reading(valid());
    await retry;
    fixture.dispatch.mockReset();
    fixture.backend.currentFrame(202, view);
    expect(fixture.dispatch).toHaveBeenCalledExactlyOnceWith({ type: "focus", controlId: "title-toggle" });
  });

  it("rejects stale permission completion without changing the fresh attempt", async () => {
    const fixture = createFixture();
    let resolvePermission: (value: PhoneVrPermissionResult) => void = () => undefined;
    const delayed = new Promise<PhoneVrPermissionResult>((resolve) => { resolvePermission = resolve; });
    fixture.sensors.requestPermissionFromUserGesture.mockReturnValueOnce(delayed);
    const oldPermission = fixture.backend.requestPermissionFromUserGesture();
    await fixture.backend.cancelPendingRequest();
    expect(await fixture.backend.requestPermissionFromUserGesture()).toEqual({ ok: true });
    const startup = fixture.backend.start();
    const callbacks = fixture.sensors.current();
    resolvePermission({ ok: true });
    expect(await oldPermission).toMatchObject({ ok: false });
    expect(fixture.sensors.current()).toBe(callbacks);
    callbacks.reading(valid());
    await startup;
    expect(fixture.backend.currentFrame(200, view).panelVisible).toBe(true);
  });

  it("returns runtime to Screen on synchronous startup failure and on a later active failure", async () => {
    const fixture = createFixture();
    const screen = new ScreenPresentationBackend(viewport);
    const runtime = new PresentationRuntime(fixture.renderer, [screen, fixture.backend], () => view);
    const recoveries: ReturnType<PresentationRuntime["switchTo"]>[] = [];
    fixture.unavailable.mockImplementation(() => { recoveries.push(runtime.switchTo("screen")); });
    expect(await runtime.start("screen")).toEqual({ ok: true });
    fixture.sensors.onStart = (callbacks) => {
      callbacks.reading(valid());
      callbacks.rotation(null);
    };
    await fixture.backend.requestPermissionFromUserGesture();
    const failedStart = await runtime.switchTo("phone-vr");
    if (failedStart.ok || failedStart.error.type !== "backend-failed") throw new Error("Expected Phone VR startup failure");
    expect(failedStart.error.mode).toBe("phone-vr");
    expect(failedStart.error.message).toContain("screen orientation became unavailable");
    expect(runtime.currentMode).toBe("screen");
    expect(fixture.stereo()).toBeNull();
    fixture.sensors.onStart = (callbacks) => { callbacks.reading(valid()); };
    await fixture.backend.requestPermissionFromUserGesture();
    expect(await runtime.switchTo("phone-vr")).toEqual({ ok: true });
    const current = fixture.sensors.current();
    current.reading({ ...valid(), gamma: null });
    current.rotation(null);
    expect(fixture.unavailable).toHaveBeenCalledTimes(1);
    expect(recoveries).toHaveLength(1);
    expect(await recoveries[0]).toEqual({ ok: true });
    expect(runtime.currentMode).toBe("screen");
    await runtime.dispose();
  });
});

class StartupSensors implements PhoneVrSensorPort {
  private callbacks: SensorCallbacks | null = null;
  onStart: (callbacks: SensorCallbacks) => void = () => undefined;
  readonly checkAvailability = vi.fn(() => Promise.resolve({ supported: true, message: "test sensor" }));
  readonly requestPermissionFromUserGesture = vi.fn((): Promise<PhoneVrPermissionResult> => Promise.resolve({ ok: true }));
  readonly getScreenOrientationAngle = vi.fn((): number | null => 0);
  readonly startListening = vi.fn((reading: SensorCallbacks["reading"], rotation: SensorCallbacks["rotation"]) => {
    this.callbacks = { reading, rotation };
    this.onStart(this.callbacks);
  });
  readonly stopListening = vi.fn(() => { this.detach(); });

  current(): SensorCallbacks {
    if (this.callbacks === null) throw new Error("Sensor listener is detached");
    return this.callbacks;
  }

  detach(): void { this.callbacks = null; }
}

function createFixture() {
  const sensors = new StartupSensors();
  let profile: StereoPresentationProfile | null = null;
  const renderer = {
    startLoop: vi.fn<RendererAdapter["startLoop"]>(),
    stopLoop: vi.fn<RendererAdapter["stopLoop"]>(),
    render: vi.fn<RendererAdapter["render"]>(),
    setFlightPose: vi.fn<RendererAdapter["setFlightPose"]>(),
    setLakeVisualCondition: vi.fn<RendererAdapter["setLakeVisualCondition"]>(),
    setFlightCameraMode: vi.fn<RendererAdapter["setFlightCameraMode"]>(),
    setCinematicCameraView: vi.fn<RendererAdapter["setCinematicCameraView"]>(),
    transformTrackingPose: vi.fn<RendererAdapter["transformTrackingPose"]>((pose) => pose),
    resize: vi.fn<RendererAdapter["resize"]>(),
    setStereoPresentation: vi.fn<RendererAdapter["setStereoPresentation"]>((value) => { profile = value; }),
    setSelectRayHandler: vi.fn<RendererAdapter["setSelectRayHandler"]>(),
    dispose: vi.fn<RendererAdapter["dispose"]>()
  } satisfies RendererAdapter;
  const unavailable = vi.fn<(message: string) => void>();
  const dispatch = vi.fn<UiActionDispatcher>();
  const gamepad = vi.fn((): PhoneVrGamepadState | null => null);
  const clock = vi.fn(() => 200);
  const backend = new PhoneVrPresentationBackend(sensors, renderer, viewport, dispatch, unavailable, {
    firstSampleTimeoutMs: 50, nowMs: clock, gamepadInput: { readState: gamepad }
  });
  backends.push(backend);
  return { sensors, renderer, unavailable, dispatch, gamepad, backend, clock, stereo: () => profile };
}

function expectReleased(fixture: ReturnType<typeof createFixture>): void {
  expect(fixture.sensors.stopListening).toHaveBeenCalledTimes(1);
  expect(() => fixture.sensors.current()).toThrow("detached");
  expect(fixture.stereo()).toBeNull();
  expect(vi.getTimerCount()).toBe(0);
}

function observe(promise: Promise<void>) {
  let result: { readonly type: "pending" | "resolved" } | { readonly type: "rejected"; readonly error: unknown } = { type: "pending" };
  void promise.then(() => { result = { type: "resolved" }; }, (error: unknown) => { result = { type: "rejected", error }; });
  return { result: () => result };
}

function expectRejected(result: ReturnType<ReturnType<typeof observe>["result"]>): Error {
  expect(result.type).toBe("rejected");
  if (result.type !== "rejected" || !(result.error instanceof Error)) throw new Error("Expected rejected startup Error");
  return result.error;
}

async function flushMicrotasks(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

async function end(backend: PhoneVrPresentationBackend, operation: "stop" | "cancel"): Promise<void> {
  if (operation === "stop") await backend.stop();
  else await backend.cancelPendingRequest();
}
