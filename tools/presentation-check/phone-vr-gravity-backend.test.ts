import { configuredViewerFixture, visiblePanelFrame } from "./viewer-fixture.js";
import { fixtureBackendFrame, fixturePresentation } from "./menu-fixture.js";
import { afterEach, describe, expect, it, vi } from "vitest";
import { Euler, Matrix4, Quaternion as ThreeQuaternion, Vector3 } from "three";
import { composePose, IDENTITY_POSE, pose, quaternion, vec3 } from "../../web/src/render/contracts/math.js";
import type { Pose } from "../../web/src/render/contracts/math.js";
import type { RendererAdapter, StereoPresentationProfile } from "../../web/src/render/contracts/runtime.js";
import { createAnchorFixture, createSceneFixture } from "../../web/src/presentation/fixtures.js";
import { PhoneVrPresentationBackend } from "../../web/src/presentation/phone-vr-backend.js";
import { createBrowserPhoneVrSensorPort } from "../../web/src/presentation/phone-vr-browser.js";
import type { PhoneVrSensorPort, PhoneVrSensorReading } from "../../web/src/presentation/phone-vr-contracts.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppMessage } from "../../web/src/app/app-state.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { ScreenPresentationBackend } from "../../web/src/presentation/screen-backend.js";

const RADIANS = Math.PI / 180;
const SCREEN_ANGLES = [0, 90, -90, 180, 270];
const backends: PhoneVrPresentationBackend[] = [];
const title = createSceneFixture("Title");

afterEach(async () => {
  for (const backend of backends.splice(0)) await backend.stop();
  vi.useRealTimers();
});

describe("Phone gravity browser/backend connection", () => {
  it.each(SCREEN_ANGLES)("preserves initial gravity tilt and vertical return at screen %s", async (screenAngle) => {
    for (const heading of [-170, 0, 75]) {
      for (const pitch of [-30, 0, 30]) {
        for (const roll of [-20, 0, 20]) {
          const fixture = createFixture(screenAngle);
          const startup = await begin(fixture.backend);
          fixture.browser.emit(head(heading, pitch, roll));
          await startup.promise;
          expectRotation(fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).cameraPose, head(0, pitch, roll));
          fixture.browser.emit(head(heading, 0, 0));
          expectRotation(fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).cameraPose, new Matrix4());
          expect(fixture.unavailable).toEqual([]);
          await fixture.backend.stop();
          expect(fixture.browser.listenerCount()).toBe(0);
        }
      }
    }
  });

  it.each(SCREEN_ANGLES)("keeps pitch/roll and mounted anchor relationships on yaw-only recenter at screen %s", async (screenAngle) => {
    const mount = pose(vec3(0.3, 1.1, -0.2), quaternion(Math.cos(0.15), 0, 0, Math.sin(0.15)));
    const fixture = createFixture(screenAngle, mount);
    const startup = await begin(fixture.backend);
    fixture.browser.emit(head(-40, -30, -20));
    await startup.promise;
    fixture.browser.emit(head(75, 30, 20));
    const before = new Map<string, Matrix4>();
    for (const anchor of ["menu", "world", "cockpit", "head"] as const) {
      const frame = fixtureBackendFrame(fixture.backend, 100, { ...title, panels: [createAnchorFixture(anchor)] }, configuredViewerFixture());
      before.set(anchor, matrix(composePose(mount, frame.cameraPose)).invert().multiply(matrix(visiblePanelFrame(frame).pose)));
    }
    fixture.backend.recenterTracking();
    for (const anchor of ["menu", "world", "cockpit", "head"] as const) {
      const frame = fixtureBackendFrame(fixture.backend, 100, { ...title, panels: [createAnchorFixture(anchor)] }, configuredViewerFixture());
      expectRotation(frame.cameraPose, head(0, 30, 20));
      const expected = before.get(anchor);
      if (expected === undefined) throw new Error("Missing anchor relationship");
      expectMatrix(matrix(composePose(mount, frame.cameraPose)).invert().multiply(matrix(visiblePanelFrame(frame).pose)), expected);
    }
    const current = fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).cameraPose;
    fixture.backend.recenterMenu();
    expect(fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).cameraPose).toEqual(current);
    fixture.browser.emit(head(90, -30, -20));
    expectRotation(fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).cameraPose, head(15, -30, -20));
  });

  it("waits for gravity evidence and a nondegenerate heading without manufacturing an identity viewer", async () => {
    const fixture = createFixture(0);
    const startup = await begin(fixture.backend);
    fixture.browser.emit(head(10, 30, 20), false, "deviceorientation");
    await Promise.resolve();
    expect(startup.completed()).toBe(false);
    expect((fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).panel.kind === "visible")).toBe(false);
    fixture.browser.emit(head(10, 90, 20));
    await Promise.resolve();
    expect(startup.completed()).toBe(false);
    fixture.browser.emit(head(10, 30, 20));
    await startup.promise;
    expectRotation(fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).cameraPose, head(0, 30, 20));
    fixture.browser.emit(head(0, 0, 0), false, "deviceorientation");
    expect(fixture.unavailable).toEqual([]);
    expectRotation(fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).cameraPose, head(0, 30, 20));
  });

  it("reports a relative-only source as an unsuccessful startup and releases both orientation listeners", async () => {
    vi.useFakeTimers();
    const fixture = createFixture(0);
    const startup = await begin(fixture.backend);
    const rejected = expect(startup.promise).rejects.toThrow("gravity and a usable horizontal heading");
    fixture.browser.emit(head(0, 30, 20), false, "deviceorientation");
    await vi.advanceTimersByTimeAsync(20);
    await rejected;
    expect(startup.completed()).toBe(false);
    expect(fixture.renderer.stereo).toBeNull();
    expect(fixture.browser.listenerCount()).toBe(0);
    expect(fixture.unavailable).toEqual([]);
  });

  it("retains active calibration through both poles and declines degenerate recenter without a flip", async () => {
    const fixture = createFixture(90);
    const startup = await begin(fixture.backend);
    fixture.browser.emit(head(20, 0, 0));
    await startup.promise;
    for (const pitch of [89.5, 90, 90.5, 120, -89.5, -90, -90.5, -120]) {
      fixture.browser.emit(head(65, pitch, 20));
      const before = fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture());
      expectRotation(before.cameraPose, head(45, pitch, 20));
      if (Math.abs(pitch) <= 90) {
        fixture.backend.recenterTracking();
        expect(fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture())).toEqual(before);
      }
    }
    fixture.browser.emit(head(65, 30, 20));
    fixture.backend.recenterTracking();
    expectRotation(fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).cameraPose, head(0, 30, 20));
  });

  it("keeps the Model in starting until relative-only startup fails and Screen recovery completes", async () => {
    vi.useFakeTimers();
    const fixture = createFixture(0);
    const runtime = new PresentationRuntime(fixture.renderer, [new ScreenPresentationBackend(viewport), fixture.backend], () => fixturePresentation(title));
    let model = createInitialAppModel();
    const dispatch = (message: AppMessage) => { model = updateApp(model, message).model; };
    expect(await runtime.start("screen")).toEqual({ ok: true });
    dispatch({ type: "initialize" });
    dispatch({ type: "presentation-initialized", requestId: 1, activeMode: runtime.currentMode,
      webXrAvailable: false, phoneVrAvailable: true, status: "Screen is active" });
    dispatch({ type: "ui-action", action: { type: "activate", controlId: "boot-enter-phone-vr" } });
    expect(model.presentation).toMatchObject({ type: "transitioning", phase: "requesting", to: "phone-vr" });
    expect(await fixture.backend.requestPermissionFromUserGesture()).toEqual({ ok: true });
    dispatch({ type: "permission-completed", requestId: 2, mode: "phone-vr", ok: true, message: "" });
    const startup = runtime.switchTo("phone-vr");
    await vi.advanceTimersByTimeAsync(0);
    fixture.browser.emit(head(30, -30, 20), false, "deviceorientation");
    expect(model.presentation).toMatchObject({ type: "transitioning", phase: "starting", to: "phone-vr" });
    expect(model.status).toBe("Waiting for gravity-referenced orientation.");
    expect(runtime.currentMode).toBeNull();
    await vi.advanceTimersByTimeAsync(20);
    const result = await startup;
    expect(result.ok).toBe(false);
    if (result.ok || result.error.type !== "backend-failed") throw new Error("Missing Phone startup failure");
    dispatch({ type: "backend-transition-completed", requestId: 2, requestedMode: "phone-vr",
      activeMode: runtime.currentMode, ok: false, message: result.error.message, successStatus: "Phone VR is active" });
    expect(model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(model.status).toContain("gravity and a usable horizontal heading");
    expect(model.status).toContain("Screen is active");
    expect(fixture.renderer.stereo).toBeNull();
    expect(fixture.browser.listenerCount()).toBe(0);
    expect(await runtime.dispose()).toEqual({ ok: true });
  });

  it.each(["deviceorientation", "deviceorientationabsolute", "change"])("attempts every browser cleanup when %s removal throws and invalidates retained callbacks", (failure) => {
    const browser = createBrowser(0, failure);
    const sensors = createBrowserPhoneVrSensorPort(browser.window);
    const readings: PhoneVrSensorReading[] = [];
    sensors.startListening((reading) => { readings.push(reading); }, () => undefined);
    const released = browser.captureListeners();
    expect(() => { sensors.stopListening(); }).toThrow("listener removal failed");
    expect(browser.removals).toEqual(["deviceorientation", "deviceorientationabsolute", "change"]);
    for (const listener of released) listener({ alpha: 0, beta: 90, gamma: 0, timeStamp: 1, absolute: true });
    expect(readings).toEqual([]);
    sensors.startListening((reading) => { readings.push(reading); }, () => undefined);
    for (const listener of released) listener({ alpha: 0, beta: 90, gamma: 0, timeStamp: 1, absolute: true });
    expect(readings).toEqual([]);
    browser.emit(head(0, 0, 0));
    expect(readings).toHaveLength(1);
    sensors.stopListening();
    expect(browser.listenerCount()).toBe(0);
  });

  it.each(["screen-first", "device-first"] as const)("preserves the final physical pose with %s screen rotation events", async (order) => {
    const fixture = createFixture(0);
    const startup = await begin(fixture.backend);
    fixture.browser.emit(head(50, -30, 20));
    await startup.promise;
    for (const angle of SCREEN_ANGLES) {
      if (order === "screen-first") fixture.browser.rotateScreen(angle);
      fixture.browser.emit(head(65, 30, -20), true, "deviceorientationabsolute", angle);
      if (order === "device-first") fixture.browser.rotateScreen(angle);
      expectRotation(fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).cameraPose, head(15, 30, -20));
    }
  });

  it("rejects a lost selected gravity source and permits a clean reentry while ignoring released adapter callbacks", async () => {
    const fixture = createFixture(0);
    const startup = await begin(fixture.backend);
    fixture.browser.emit(head(10, -30, 20));
    await startup.promise;
    const released = fixture.browser.captureListeners();
    fixture.browser.emit(head(10, -30, 20), false);
    expect(fixture.unavailable).toEqual(["Phone VR gravity reference became unavailable or changed"]);
    expect(fixture.renderer.stereo).toBeNull();
    expect(fixture.browser.listenerCount()).toBe(0);
    const retry = await begin(fixture.backend);
    for (const listener of released) listener({ alpha: 0, beta: 90, gamma: 0, timeStamp: 1, absolute: true });
    await Promise.resolve();
    expect(retry.completed()).toBe(false);
    fixture.browser.emit(head(75, 30, -20));
    await retry.promise;
    expectRotation(fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).cameraPose, head(0, 30, -20));
  });

  it("keeps a canceled pending gravity attempt separate from a later session", async () => {
    const fixture = createFixture(0);
    const initial = await begin(fixture.backend);
    const rejected = expect(initial.promise).rejects.toThrow("canceled");
    const oldListeners = fixture.browser.captureListeners();
    fixture.browser.emit(head(0, 90, 0));
    await fixture.backend.cancelPendingRequest();
    await rejected;
    const retry = await begin(fixture.backend);
    for (const listener of oldListeners) listener({ alpha: 0, beta: 90, gamma: 0, timeStamp: 1, absolute: true });
    await Promise.resolve();
    expect(retry.completed()).toBe(false);
    fixture.browser.emit(head(-20, -30, 20));
    await retry.promise;
    expectRotation(fixtureBackendFrame(fixture.backend, 100, title, configuredViewerFixture()).cameraPose, head(0, -30, 20));
  });

  it("accepts a caller-confirmed relative reference but rejects changes to that reference", async () => {
    const referenceFromEarth = new Matrix4().makeRotationX(0.4).multiply(new Matrix4().makeRotationZ(-0.3));
    const referenceUp = new Vector3(0, 0, 1).applyMatrix4(referenceFromEarth);
    let send: ((reading: PhoneVrSensorReading) => void) | null = null;
    const sensors: PhoneVrSensorPort = {
      checkAvailability: () => Promise.resolve({ supported: true, message: "Confirmed reference up" }),
      requestPermissionFromUserGesture: () => Promise.resolve({ ok: true }),
      getScreenOrientationAngle: () => 0,
      startListening: (reading) => { send = reading; },
      stopListening: () => { send = null; }
    };
    const unavailable: string[] = [];
    const backend = new PhoneVrPresentationBackend(sensors, new RecordingRenderer(), viewport, () => undefined,
      (message) => { unavailable.push(message); }, { nowMs: () => 1000 });
    backends.push(backend);
    const startup = await begin(backend);
    const emit = send as ((reading: PhoneVrSensorReading) => void) | null;
    if (emit === null) throw new Error("Missing relative source listener");
    const angles = anglesFor(head(75, 30, -20), 0, referenceFromEarth);
    emit({ ...angles, timestampMs: 10, gravityEvidence: { kind: "relative-reference-up", referenceUp } });
    await startup.promise;
    expectRotation(fixtureBackendFrame(backend, 100, title, configuredViewerFixture()).cameraPose, head(0, 30, -20));
    emit({ ...angles, timestampMs: 11, gravityEvidence: { kind: "relative-reference-up", referenceUp: { x: 0, y: 0, z: 0 } } });
    expect(unavailable).toEqual(["Phone VR gravity reference became unavailable or changed"]);
  });
});

async function begin(backend: PhoneVrPresentationBackend) {
  expect(await backend.requestPermissionFromUserGesture()).toEqual({ ok: true });
  let completed = false;
  const promise = backend.start().then(() => { completed = true; });
  return { promise, completed: () => completed };
}

function createFixture(screenAngle: number, mount: Pose = IDENTITY_POSE) {
  const browser = createBrowser(screenAngle);
  const renderer = new RecordingRenderer(mount);
  const unavailable: string[] = [];
  const backend = new PhoneVrPresentationBackend(createBrowserPhoneVrSensorPort(browser.window), renderer, viewport,
    () => undefined, (message) => { unavailable.push(message); }, { nowMs: () => 1000, firstSampleTimeoutMs: 20 });
  backends.push(backend);
  return { browser, renderer, backend, unavailable };
}

class RecordingRenderer implements RendererAdapter {
  beginViewFrame(): void {}
  stereo: StereoPresentationProfile | null = null;
  constructor(private readonly mount: Pose = IDENTITY_POSE) {}
  startLoop(): void {}
  stopLoop(): void {}
  render(): void {}
  setFlightPose(): void {}
  setLakeVisualCondition(): void {}
  setFlightCameraMode(): void {}
  setCinematicCameraView(): void {}
  transformTrackingPose(value: Pose): Pose { return composePose(this.mount, value); }
  resize(): void {}
  setStereoPresentation(value: StereoPresentationProfile | null): void { this.stereo = value; }
  setSelectRayHandler(): void {}
  dispose(): void {}
}

type ReadingEvent = { readonly alpha: number; readonly beta: number; readonly gamma: number; readonly timeStamp: number; readonly absolute: boolean };

function createBrowser(initialScreen: number, removalFailure: string | null = null) {
  const listeners = new Map<string, (event: ReadingEvent) => void>();
  const screenListeners = new Set<() => void>();
  let screen = initialScreen;
  let timestamp = 0;
  const removals: string[] = [];
  const remove = (type: string) => {
    removals.push(type);
    if (type === removalFailure) {
      removalFailure = null;
      throw new Error("Simulated browser removal failure");
    }
  };
  return {
    removals,
    window: {
      isSecureContext: true,
      DeviceOrientationEvent: { requestPermission: (absolute?: boolean) => {
        expect(absolute).toBe(true);
        return Promise.resolve("granted" as const);
      } },
      screen: { orientation: {
        get angle() { return screen; },
        addEventListener: (_type: string, listener: () => void) => { screenListeners.add(listener); },
        removeEventListener: (type: string, listener: () => void) => { screenListeners.delete(listener); remove(type); }
      } },
      addEventListener: (type: string, listener: (event: ReadingEvent) => void) => { listeners.set(type, listener); },
      removeEventListener: (type: string) => { listeners.delete(type); remove(type); }
    },
    emit(value: Matrix4, absolute = true, type = "deviceorientationabsolute", deviceScreen = screen): void {
      timestamp++;
      listeners.get(type)?.({ ...anglesFor(value, deviceScreen), absolute, timeStamp: timestamp });
    },
    rotateScreen(value: number): void {
      screen = value;
      for (const listener of screenListeners) listener();
    },
    captureListeners: () => [...listeners.values()],
    listenerCount: () => listeners.size + screenListeners.size
  };
}

function anglesFor(value: Matrix4, screen: number, referenceFromEarth = new Matrix4()) {
  const device = referenceFromEarth.clone().multiply(new Matrix4().makeRotationX(Math.PI / 2))
    .multiply(value).multiply(new Matrix4().makeRotationZ(screen * RADIANS));
  const angles = new Euler().setFromRotationMatrix(device, "ZXY");
  return { alpha: angles.z / RADIANS, beta: angles.x / RADIANS, gamma: angles.y / RADIANS };
}

function head(heading: number, pitch: number, roll: number): Matrix4 {
  return new Matrix4().makeRotationY(heading * RADIANS).multiply(new Matrix4().makeRotationX(pitch * RADIANS))
    .multiply(new Matrix4().makeRotationZ(roll * RADIANS));
}

function viewport() { return { x: 1280, y: 720, pixelRatio: 1 }; }

function matrix(value: Pose): Matrix4 {
  return new Matrix4().compose(new Vector3(value.position.x, value.position.y, value.position.z),
    new ThreeQuaternion(value.orientation.x, value.orientation.y, value.orientation.z, value.orientation.w), new Vector3(1, 1, 1));
}

function expectRotation(actual: Pose, expected: Matrix4): void {
  expectMatrix(matrix({ ...actual, position: vec3(0, 0, 0) }), expected);
}

function expectMatrix(actual: Matrix4, expected: Matrix4): void {
  for (const [index, value] of actual.elements.entries()) expect(value).toBeCloseTo(expected.elements[index] ?? NaN, 10);
}
