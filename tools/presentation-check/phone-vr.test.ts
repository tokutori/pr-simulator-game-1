import { describe, expect, it, vi } from "vitest";
import { IDENTITY_POSE, quaternion } from "../../web/src/render/contracts/math.js";
import type { BackendFrame, RendererAdapter, StereoPresentationProfile, ViewportSize } from "../../web/src/render/contracts/runtime.js";
import type { UiAction, UiViewModel } from "../../web/src/render/contracts/ui.js";
import { createAnchorFixture, createSceneFixture, SCENE_FIXTURE_OVERLAYS } from "../../web/src/presentation/fixtures.js";
import { createBrowserPhoneVrSensorPort } from "../../web/src/presentation/phone-vr-browser.js";
import { createBrowserPhoneVrGamepadInputPort } from "../../web/src/presentation/phone-vr-gamepad-browser.js";
import { PhoneVrPresentationBackend } from "../../web/src/presentation/phone-vr-backend.js";
import { GamepadUiSelector } from "../../web/src/presentation/gamepad-ui-selector.js";
import type { PhoneVrAvailability, PhoneVrGamepadInputPort, PhoneVrPermissionResult, PhoneVrSensorPort, PhoneVrSensorReading } from "../../web/src/presentation/phone-vr-contracts.js";
import { PHONE_VR_OPTICAL_PROFILE } from "../../web/src/presentation/phone-vr-contracts.js";
import { phoneOrientationQuaternion } from "../../web/src/presentation/phone-vr-orientation.js";
import { GAME_SCENES } from "../../web/src/render/contracts/ui.js";
import { createInitialAppModel, updateApp } from "../../web/src/app/app-state.js";
import type { AppEffect, AppMessage } from "../../web/src/app/app-state.js";
import { PresentationRuntime } from "../../web/src/presentation/runtime.js";
import { ScreenPresentationBackend } from "../../web/src/presentation/screen-backend.js";

describe("Phone VR device orientation", () => {
  it("converts the W3C intrinsic Z-X'-Y'' angles and screen rotation", () => {
    expect(phoneOrientationQuaternion(reading(0, 0, 0), 0)).toEqual(IDENTITY_POSE.orientation);
    expect(phoneOrientationQuaternion(reading(90, 0, 0), 0)).toEqual(axisQuaternion("z", 90));
    expect(phoneOrientationQuaternion(reading(0, 90, 0), 0)).toEqual(axisQuaternion("x", 90));
    expect(phoneOrientationQuaternion(reading(0, 0, 90), 0)).toEqual(axisQuaternion("y", 90));
    expect(phoneOrientationQuaternion(reading(0, 0, 0), 90)).toEqual(axisQuaternion("z", -90));
    expect(phoneOrientationQuaternion(reading(0, 0, 0), -90)).toEqual(axisQuaternion("z", 90));
  });

  it("rejects null and non-finite sensor data instead of substituting zero", () => {
    expect(phoneOrientationQuaternion({ ...reading(0, 0, 0), alpha: null }, 0)).toBeNull();
    expect(phoneOrientationQuaternion(reading(0, Number.NaN, 0), 0)).toBeNull();
    expect(phoneOrientationQuaternion(reading(0, 0, 0), Number.POSITIVE_INFINITY)).toBeNull();
  });
});

describe("Phone VR browser sensor adapter", () => {
  it("requests permission synchronously, preserves null angles, reports rotation, and removes listeners", async () => {
    const browser = createMockBrowserWindow();
    const sensors = createBrowserPhoneVrSensorPort(browser.window);
    expect(await sensors.checkAvailability()).toMatchObject({ supported: true });
    const permission = sensors.requestPermissionFromUserGesture();
    expect(browser.permissionRequests).toEqual([false]);
    expect(await permission).toEqual({ ok: true });

    const readings: PhoneVrSensorReading[] = [];
    const screenAngles: Array<number | null> = [];
    sensors.startListening((value) => { readings.push(value); }, (angle) => { screenAngles.push(angle); });
    browser.emitDeviceOrientation({ alpha: null, beta: 20, gamma: -10, timestampMs: 42 });
    browser.setScreenAngle(90);
    browser.emitScreenOrientationChange();
    expect(readings).toEqual([{ alpha: null, beta: 20, gamma: -10, timestampMs: 42 }]);
    expect(screenAngles).toEqual([90]);

    sensors.stopListening();
    browser.emitDeviceOrientation({ alpha: 0, beta: 0, gamma: 0, timestampMs: 50 });
    browser.emitScreenOrientationChange();
    expect(readings).toHaveLength(1);
    expect(screenAngles).toHaveLength(1);
  });

  it("reports insecure contexts and missing sensor APIs", async () => {
    const insecure = createMockBrowserWindow({ secure: false });
    expect(await createBrowserPhoneVrSensorPort(insecure.window).checkAvailability()).toMatchObject({ supported: false });
    const missingOrientation = createMockBrowserWindow({ orientationApi: false });
    expect(await createBrowserPhoneVrSensorPort(missingOrientation.window).checkAvailability()).toMatchObject({ supported: false });
    const missingScreen = createMockBrowserWindow({ screenOrientationApi: false });
    expect(await createBrowserPhoneVrSensorPort(missingScreen.window).checkAvailability()).toMatchObject({ supported: false });
  });
});

describe("Phone VR gamepad UI", () => {
  it("moves a cursor, focuses controls, selects, backs, and scrolls through common actions", () => {
    const actions: UiAction[] = [];
    const selector = new GamepadUiSelector((action) => { actions.push(action); });
    const panel = createSceneFixture("Title").panels[0];
    if (panel === undefined) throw new Error("Missing UI panel fixture");
    selector.update(panel, gamepadState([0, 0, 0, 0], [false, false]), 100);
    let cursor = null;
    for (let frame = 1; frame <= 13; frame++) {
      cursor = selector.update(panel, gamepadState([0, 1, 0, 0], [false, false]), 100 + frame * 16);
    }
    expect(cursor?.point.y).toBeCloseTo(-0.449, 2);
    expect(actions).toContainEqual({ type: "focus", controlId: "title-action" });

    selector.update(panel, gamepadState([0, 0, 0, 0], [true, false]), 324);
    selector.update(panel, gamepadState([0, 0, 0, 0.5], [true, true]), 340);
    expect(actions).toContainEqual({ type: "activate", controlId: "title-action" });
    expect(actions).toContainEqual({ type: "back" });
    expectScrollAction(actions);
    expect(cursor?.progress).toBe(0);

    selector.reset();
    actions.length = 0;
    selector.update(panel, gamepadState([0, 0, 0, 0], [true, false]), 400);
    selector.update(panel, gamepadState([0, 0, 0, 0], [true, false]), 416);
    expect(actions).not.toContainEqual({ type: "set-toggle", controlId: "title-toggle", value: true });
    selector.update(panel, gamepadState([0, 0, 0, 0], [false, false]), 432);
    selector.update(panel, gamepadState([0, 0, 0, 0], [true, false]), 448);
    expect(actions).toContainEqual({ type: "set-toggle", controlId: "title-toggle", value: true });
  });

  it("reads only connected standard-mapped browser gamepads", () => {
    let currentGamepads: readonly unknown[] = [
      { connected: true, mapping: "", axes: [1, 1], buttons: [{ pressed: true }] },
      { connected: true, mapping: "standard", axes: [0.5, -0.25, 0, 1], buttons: [{ pressed: true }, { pressed: false }] }
    ];
    const input = createBrowserPhoneVrGamepadInputPort({
      getGamepads: () => currentGamepads as never
    });
    expect(input.readState()).toEqual({
      axes: [0.5, -0.25, 0, 1],
      buttons: [true, false]
    });
    currentGamepads = [{ connected: false, mapping: "standard", axes: [], buttons: [] }];
    expect(input.readState()).toBeNull();
    expect(createBrowserPhoneVrGamepadInputPort({}).readState()).toBeNull();
    expect(createBrowserPhoneVrGamepadInputPort({ getGamepads: () => { throw new Error("blocked"); } }).readState()).toBeNull();
  });
});

describe("Phone VR presentation backend", () => {
  it("uses the current performance clock by default instead of the frame timestamp", async () => {
    const clock = vi.spyOn(performance, "now").mockReturnValue(1003);
    const sensors = new FakePhoneVrSensors();
    const unavailable: string[] = [];
    const backend = new PhoneVrPresentationBackend(
      sensors, new FakeRenderer(), viewport, () => undefined,
      (message) => { unavailable.push(message); }
    );
    try {
      await backend.requestPermissionFromUserGesture();
      const startup = backend.start();
      sensors.emit(reading(0, 0, 0, 1002));
      await startup;
      expect(backend.currentFrame(1000, createSceneFixture("Title")).panelVisible).toBe(true);
      expect(unavailable).toEqual([]);
      expect(clock).toHaveBeenCalled();
    } finally {
      await backend.stop();
      clock.mockRestore();
    }
  });

  it("keeps Phone VR active when browser sensor events are newer than the rendering opportunity", async () => {
    const browser = createMockBrowserWindow();
    const renderer = new FakeRenderer();
    const unavailable: string[] = [];
    let currentTimeMs = 1003;
    let model = createInitialAppModel();
    const screenRecoveries: Promise<void>[] = [];
    const dispatch = (message: AppMessage): readonly AppEffect[] => {
      const transition = updateApp(model, message);
      model = transition.model;
      return transition.effects;
    };
    const backend = new PhoneVrPresentationBackend(
      createBrowserPhoneVrSensorPort(browser.window), renderer, viewport, () => undefined,
      (message) => {
        unavailable.push(message);
        const effects = dispatch({ type: "backend-ended", mode: "phone-vr", message });
        const recovery = effects[0];
        if (recovery?.type !== "switch-backend" || recovery.mode !== "screen") {
          throw new Error("Missing Screen recovery effect");
        }
        screenRecoveries.push(runtime.switchTo(recovery.mode).then((result) => {
          expect(result).toEqual({ ok: true });
          dispatch({
            type: "backend-transition-completed", requestId: recovery.requestId,
            requestedMode: recovery.mode, activeMode: runtime.currentMode, ok: result.ok,
            message: "", successStatus: "Screen is active"
          });
        }));
      },
      { nowMs: () => currentTimeMs }
    );
    const runtime = new PresentationRuntime(
      renderer, [new ScreenPresentationBackend(viewport), backend], () => createSceneFixture("Title")
    );
    expect(await runtime.start("screen")).toEqual({ ok: true });
    dispatch({ type: "initialize" });
    dispatch({
      type: "presentation-initialized", requestId: 1, activeMode: runtime.currentMode,
      webXrAvailable: false, phoneVrAvailable: true, status: "Screen is active"
    });
    expect(dispatch({ type: "ui-action", action: { type: "activate", controlId: "boot-enter-phone-vr" } }))
      .toEqual([{ type: "request-permission", mode: "phone-vr", requestId: 2 }]);
    expect(await backend.requestPermissionFromUserGesture()).toEqual({ ok: true });
    expect(dispatch({ type: "permission-completed", requestId: 2, mode: "phone-vr", ok: true, message: "" }))
      .toEqual([{ type: "switch-backend", mode: "phone-vr", requestId: 2 }]);
    const startup = runtime.switchTo("phone-vr");
    await browser.orientationListening;
    browser.emitDeviceOrientation(reading(0, 0, 0, 1002));
    expect(await startup).toEqual({ ok: true });
    dispatch({
      type: "backend-transition-completed", requestId: 2, requestedMode: "phone-vr",
      activeMode: runtime.currentMode, ok: true, message: "", successStatus: "Phone VR is active"
    });

    renderer.tick(1000);
    expect(unavailable).toEqual([]);
    expect(runtime.currentMode).toBe("phone-vr");
    expect(model.presentation).toEqual({ type: "ready", mode: "phone-vr" });
    expect(renderer.stereoProfile).toEqual(PHONE_VR_OPTICAL_PROFILE);
    expect(renderer.frames.at(-1)).toMatchObject({ timestampMs: 1000, panelVisible: true });

    currentTimeMs = 1018;
    browser.emitDeviceOrientation(reading(0, 20, 0, 1018));
    browser.emitDeviceOrientation(reading(0, 25, 0, 1018));
    renderer.tick(1016);
    expect(renderer.frames.at(-1)?.cameraPose.orientation).not.toEqual(IDENTITY_POSE.orientation);
    expect(unavailable).toEqual([]);
    expect(model.presentation).toEqual({ type: "ready", mode: "phone-vr" });

    const heldOrientation = renderer.frames.at(-1)?.cameraPose.orientation;
    for (const silentDurationMs of [1001, 60_000]) {
      currentTimeMs = 1018 + silentDurationMs;
      renderer.tick(currentTimeMs - 2);
      expect(unavailable).toEqual([]);
      expect(runtime.currentMode).toBe("phone-vr");
      expect(model.presentation).toEqual({ type: "ready", mode: "phone-vr" });
      expect(renderer.stereoProfile).toEqual(PHONE_VR_OPTICAL_PROFILE);
      expect(renderer.frames.at(-1)?.cameraPose.orientation).toEqual(heldOrientation);
    }

    currentTimeMs = 61_020;
    browser.emitDeviceOrientation(reading(0, 35, 0, 61_019));
    renderer.tick(61_018);
    expect(renderer.frames.at(-1)?.cameraPose.orientation).not.toEqual(heldOrientation);
    backend.recenterTracking();
    renderer.tick(61_018);
    expect(renderer.frames.at(-1)?.cameraPose.orientation).toEqual(IDENTITY_POSE.orientation);
    browser.setScreenAngle(90);
    browser.emitScreenOrientationChange();
    renderer.tick(61_018);
    expect(renderer.frames.at(-1)?.cameraPose.orientation).not.toEqual(IDENTITY_POSE.orientation);
    expect(unavailable).toEqual([]);

    browser.emitDeviceOrientation({ ...reading(0, 35, 0, 61_020), gamma: null });
    expect(unavailable).toEqual(["Phone VR orientation data contains null or non-finite values"]);
    expect(model.presentation).toMatchObject({ type: "transitioning", to: "screen" });
    expect(screenRecoveries).toHaveLength(1);
    await Promise.all(screenRecoveries);
    expect(runtime.currentMode).toBe("screen");
    expect(model.presentation).toEqual({ type: "ready", mode: "screen" });
    expect(renderer.stereoProfile).toBeNull();
    browser.emitDeviceOrientation(reading(0, 0, 0, 61_020));
    browser.emitScreenOrientationChange();
    expect(unavailable).toHaveLength(1);
    expect(screenRecoveries).toHaveLength(1);
    expect(await runtime.dispose()).toEqual({ ok: true });
  });

  it.each([Number.NaN, Number.POSITIVE_INFINITY, -1, 99])(
    "rejects invalid or backward sensor timestamps: %s", async (timestampMs) => {
      const sensors = new FakePhoneVrSensors();
      const unavailable: string[] = [];
      const backend = createBackend(sensors, new FakeRenderer(), unavailable);
      await backend.requestPermissionFromUserGesture();
      const startup = backend.start();
      sensors.emit(reading(0, 0, 0, 100));
      await startup;
      sensors.emit(reading(0, 0, 0, timestampMs), 100);
      expect(unavailable).toEqual(["Phone VR orientation timestamp is invalid"]);
      await backend.stop();
    }
  );

  it.each([Number.NaN, Number.POSITIVE_INFINITY, -1])(
    "rejects invalid first-sample timestamps independently of the receipt clock: %s", async (timestampMs) => {
      const sensors = new FakePhoneVrSensors();
      const renderer = new FakeRenderer();
      const backend = createBackend(sensors, renderer);
      await backend.requestPermissionFromUserGesture();
      const startup = backend.start();
      sensors.emit(reading(0, 0, 0, timestampMs), 100);
      await expect(startup).rejects.toThrow("orientation timestamp is invalid");
      expect(renderer.stereoProfile).toBeNull();
      expect(sensors.stopCount).toBe(1);
    }
  );

  it.each([Number.NaN, Number.POSITIVE_INFINITY, -1, 99])(
    "rejects invalid receipt clocks and future samples before accepting a pose: %s", async (currentTimeMs) => {
      for (const active of [false, true]) {
        const sensors = new FakePhoneVrSensors();
        const renderer = new FakeRenderer();
        const unavailable: string[] = [];
        const backend = createBackend(sensors, renderer, unavailable);
        await backend.requestPermissionFromUserGesture();
        const startup = backend.start();
        if (active) {
          sensors.emit(reading(0, 0, 0, 100));
          await startup;
        }
        sensors.emit(reading(0, 20, 0, 100), currentTimeMs);
        if (active) {
          expect(unavailable).toEqual(["Phone VR orientation timestamp is invalid"]);
        } else {
          await expect(startup).rejects.toThrow("orientation timestamp is invalid");
          expect(unavailable).toEqual([]);
        }
        expect(sensors.stopCount).toBe(1);
        expect(renderer.stereoProfile).toBeNull();
        await backend.stop();
      }
    }
  );

  it("requests permission synchronously and enables stereo only after a valid sensor event", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const backend = createBackend(sensors, renderer);
    expect(await backend.checkAvailability()).toEqual(sensors.availability);
    const permissionRequest = backend.requestPermissionFromUserGesture();
    expect(sensors.permissionRequestCount).toBe(1);
    expect(await permissionRequest).toEqual({ ok: true });

    let started = false;
    const startup = backend.start().then(() => { started = true; });
    expect(renderer.stereoProfile).toEqual(PHONE_VR_OPTICAL_PROFILE);
    sensors.emit(reading(0, 0, 0, 100));
    await startup;
    expect(started).toBe(true);
    expect(backend.currentFrame(110, createSceneFixture("Title")).panelVisible).toBe(true);
    await backend.stop();
    expect(renderer.stereoProfile).toBeNull();
    expect(sensors.stopCount).toBe(1);
  });

  it("rejects denied permissions without starting sensors or stereo", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    sensors.permissionResult = { ok: false, message: "Phone VR sensor permission denied" };
    const backend = createBackend(sensors, renderer);
    expect(await backend.requestPermissionFromUserGesture()).toEqual(sensors.permissionResult);
    await expect(backend.start()).rejects.toThrow("explicit user permission action");
    expect(sensors.startCount).toBe(0);
    expect(renderer.stereoProfile).toBeNull();
  });

  it("cancels a startup that is waiting for its first sensor sample", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const backend = createBackend(sensors, renderer);
    await backend.requestPermissionFromUserGesture();
    const startup = backend.start();
    await backend.cancelPendingRequest();
    await expect(startup).rejects.toThrow("canceled");
    expect(renderer.stereoProfile).toBeNull();
    expect(sensors.stopCount).toBe(1);
  });

  it.each(["stop", "cancel"] as const)("ignores late readings after %s without accessing the clock", async (operation) => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const unavailable: string[] = [];
    const clock = vi.fn(() => sensors.currentTimeMs);
    const backend = new PhoneVrPresentationBackend(
      sensors, renderer, viewport, () => undefined, (message) => { unavailable.push(message); }, { nowMs: clock }
    );
    await backend.requestPermissionFromUserGesture();
    const startup = backend.start();
    const lateReading = sensors.captureReadingCallback();
    if (operation === "stop") {
      sensors.emit(reading(0, 0, 0, 100));
      await startup;
      await backend.stop();
    } else {
      await backend.cancelPendingRequest();
      await expect(startup).rejects.toThrow("canceled");
    }
    clock.mockClear();
    lateReading(reading(0, 20, 0, Number.NaN));
    lateReading(reading(0, 20, 0, 60_100));
    expect(clock).not.toHaveBeenCalled();
    expect(unavailable).toEqual([]);
    expect(renderer.stereoProfile).toBeNull();
    expect(backend.currentFrame(60_100, createSceneFixture("Title")).panelVisible).toBe(false);
  });

  it("cleans up startup after missing or null tracking", async () => {
    const timeoutSensors = new FakePhoneVrSensors();
    const timeoutRenderer = new FakeRenderer();
    const timeoutBackend = createBackend(timeoutSensors, timeoutRenderer, [], 2);
    await timeoutBackend.requestPermissionFromUserGesture();
    await expect(timeoutBackend.start()).rejects.toThrow("before timeout");
    expect(timeoutRenderer.stereoProfile).toBeNull();
    expect(timeoutSensors.stopCount).toBe(1);

    const nullSensors = new FakePhoneVrSensors();
    const nullRenderer = new FakeRenderer();
    const nullBackend = createBackend(nullSensors, nullRenderer);
    await nullBackend.requestPermissionFromUserGesture();
    const nullStartup = nullBackend.start();
    nullSensors.emit({ ...reading(0, 0, 0, 100), gamma: null });
    await expect(nullStartup).rejects.toThrow("null or non-finite");
    expect(nullRenderer.stereoProfile).toBeNull();
    expect(nullSensors.stopCount).toBe(1);
  });

  it.each([null, Number.NaN, Number.POSITIVE_INFINITY])(
    "ends tracking on an explicit invalid angle after a silent interval: %s", async (gamma) => {
      const sensors = new FakePhoneVrSensors();
      const renderer = new FakeRenderer();
      const unavailable: string[] = [];
      const backend = createBackend(sensors, renderer, unavailable);
      await backend.requestPermissionFromUserGesture();
      const startup = backend.start();
      sensors.emit(reading(0, 0, 0, 100));
      await startup;
      expect(backend.currentFrame(60_100, createSceneFixture("Title")).panelVisible).toBe(true);
      sensors.emit({ ...reading(0, 0, 0, 60_101), gamma });
      expect(unavailable).toEqual(["Phone VR orientation data contains null or non-finite values"]);
      expect(renderer.stereoProfile).toBeNull();
      expect(sensors.stopCount).toBe(1);
      sensors.emit({ ...reading(0, 0, 0, 60_102), gamma });
      expect(unavailable).toHaveLength(1);
      await backend.stop();
    }
  );

  it.each([null, Number.NaN])("ends tracking when screen orientation becomes unavailable: %s", async (angle) => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const unavailable: string[] = [];
    const backend = createBackend(sensors, renderer, unavailable);
    await backend.requestPermissionFromUserGesture();
    const startup = backend.start();
    sensors.emit(reading(0, 0, 0, 100));
    await startup;
    expect(backend.currentFrame(60_100, createSceneFixture("Title")).panelVisible).toBe(true);
    sensors.changeScreenOrientation(angle);
    expect(unavailable).toEqual(["Phone VR screen orientation became unavailable"]);
    expect(renderer.stereoProfile).toBeNull();
    expect(sensors.stopCount).toBe(1);
    await backend.stop();
  });

  it("renders and gaze-activates common controls in every scene and overlay", async () => {
    let timestampMs = 100;
    for (const scene of GAME_SCENES) {
      for (const overlay of [null, ...SCENE_FIXTURE_OVERLAYS[scene]]) {
        const sensors = new FakePhoneVrSensors();
        const renderer = new FakeRenderer();
        const actions: UiAction[] = [];
        const backend = new PhoneVrPresentationBackend(
          sensors, renderer, viewport, (action) => { actions.push(action); }, () => undefined,
          { opticalProfile: PHONE_VR_OPTICAL_PROFILE, firstSampleTimeoutMs: 3000, nowMs: () => sensors.currentTimeMs }
        );
        await backend.requestPermissionFromUserGesture();
        const startup = backend.start();
        sensors.emit(reading(0, 0, 0, timestampMs));
        await startup;
        const viewModel = withMenuAnchors(createSceneFixture(scene, overlay));
        const frame = backend.currentFrame(timestampMs + 1, viewModel);
        expect(frame.panelVisible).toBe(true);
        expect(frame.panel?.id).toBe(`${scene.toLowerCase()}-panel`);
        sensors.emit(reading(0, -10.2, 0, timestampMs + 2));
        backend.currentFrame(timestampMs + 2, viewModel);
        const activatedFrame = backend.currentFrame(timestampMs + 1002, viewModel);
        expect(activatedFrame.gazeCursor?.progress).toBe(1);
        if (!actions.some((action) => action.type === "activate" && action.controlId === `${scene.toLowerCase()}-action`)) {
          throw new Error(`Missing gaze activation for ${scene}/${String(overlay)} at ${String(timestampMs)}`);
        }
        await backend.stop();
        timestampMs += 2000;
      }
    }
  });

  it("re-centers tracking continuously and applies screen-orientation changes", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const backend = createBackend(sensors, renderer);
    await backend.requestPermissionFromUserGesture();
    const startup = backend.start();
    sensors.emit(reading(0, 0, 0, 100));
    await startup;
    const initial = backend.currentFrame(110, createSceneFixture("Title"));
    sensors.emit(reading(0, 20, 0, 120));
    const turned = backend.currentFrame(120, createSceneFixture("Title"));
    expect(turned.cameraPose.orientation).not.toEqual(initial.cameraPose.orientation);
    backend.recenterTracking();
    const recentered = backend.currentFrame(121, createSceneFixture("Title"));
    expect(recentered.cameraPose.orientation).toEqual(IDENTITY_POSE.orientation);
    sensors.changeScreenOrientation(90);
    const rotated = backend.currentFrame(122, createSceneFixture("Title"));
    expect(rotated.cameraPose.orientation).not.toEqual(IDENTITY_POSE.orientation);
    await backend.stop();
  });

  it("keeps Menu fixed, makes Head follow tracking, and maps World/Cockpit to the tracking reference", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const backend = createBackend(sensors, renderer);
    await backend.requestPermissionFromUserGesture();
    const startup = backend.start();
    sensors.emit(reading(0, 0, 0, 100));
    await startup;
    const menuBefore = backend.currentFrame(110, { ...createSceneFixture("Title"), panels: [createAnchorFixture("menu")] });
    sensors.emit(reading(0, 20, 0, 120));
    const menuAfter = backend.currentFrame(120, { ...createSceneFixture("Title"), panels: [createAnchorFixture("menu")] });
    const head = backend.currentFrame(121, { ...createSceneFixture("Title"), panels: [createAnchorFixture("head")] });
    const world = backend.currentFrame(122, { ...createSceneFixture("Title"), panels: [createAnchorFixture("world")] });
    const cockpit = backend.currentFrame(123, { ...createSceneFixture("Flight"), panels: [createAnchorFixture("cockpit")] });
    expect(menuAfter.panelPose).toEqual(menuBefore.panelPose);
    expect(head.panelPose.orientation).toEqual(head.cameraPose.orientation);
    expect(world.panelPose).toEqual(cockpit.panelPose);
    await backend.stop();
  });

  it("uses a connected gamepad for focus, selection, back, and scrolling", async () => {
    const sensors = new FakePhoneVrSensors();
    const renderer = new FakeRenderer();
    const gamepad = new FakePhoneVrGamepad();
    const actions: UiAction[] = [];
    const backend = new PhoneVrPresentationBackend(
      sensors, renderer, viewport, (action) => { actions.push(action); }, () => undefined,
      { gamepadInput: gamepad, nowMs: () => sensors.currentTimeMs }
    );
    await backend.requestPermissionFromUserGesture();
    const startup = backend.start();
    sensors.emit(reading(0, 0, 0, 100));
    await startup;
    const viewModel = withMenuAnchors(createSceneFixture("Title"));
    backend.currentFrame(110, viewModel);
    gamepad.state = gamepadState([0, 0, 0, 0], [false, false]);
    backend.currentFrame(120, viewModel);
    for (let frame = 1; frame <= 13; frame++) {
      gamepad.state = gamepadState([0, 1, 0, 0], [false, false]);
      backend.currentFrame(120 + frame * 16, viewModel);
    }
    gamepad.state = gamepadState([0, 0, 0, 0], [true, false]);
    const selected = backend.currentFrame(344, viewModel);
    expect(selected.gazeCursor?.point.y).toBeCloseTo(-0.449, 2);
    gamepad.state = gamepadState([0, 0, 0, 0.5], [false, true]);
    backend.currentFrame(360, viewModel);
    expect(actions).toContainEqual({ type: "activate", controlId: "title-action" });
    expect(actions).toContainEqual({ type: "back" });
    expectScrollAction(actions);
    await backend.stop();
  });
});

class FakePhoneVrSensors implements PhoneVrSensorPort {
  availability: PhoneVrAvailability = { supported: true, message: "Phone VR sensor API available" };
  permissionResult: PhoneVrPermissionResult = { ok: true };
  permissionRequestCount = 0;
  startCount = 0;
  stopCount = 0;
  currentTimeMs = 100;
  private readingHandler: ((value: PhoneVrSensorReading) => void) | null = null;
  private screenHandler: ((angle: number | null) => void) | null = null;
  private screenAngle: number | null = 0;

  checkAvailability(): Promise<PhoneVrAvailability> {
    return Promise.resolve(this.availability);
  }

  requestPermissionFromUserGesture(): Promise<PhoneVrPermissionResult> {
    this.permissionRequestCount++;
    return Promise.resolve(this.permissionResult);
  }

  getScreenOrientationAngle(): number | null {
    return this.screenAngle;
  }

  startListening(
    onReading: (value: PhoneVrSensorReading) => void,
    onScreenOrientationChange: (angle: number | null) => void
  ): void {
    this.startCount++;
    this.readingHandler = onReading;
    this.screenHandler = onScreenOrientationChange;
  }

  stopListening(): void {
    this.stopCount++;
    this.readingHandler = null;
    this.screenHandler = null;
  }

  emit(value: PhoneVrSensorReading, receivedTimestampMs = value.timestampMs): void {
    this.currentTimeMs = receivedTimestampMs;
    this.readingHandler?.(value);
  }

  captureReadingCallback(): (value: PhoneVrSensorReading) => void {
    if (this.readingHandler === null) throw new Error("Phone VR sensor listener is inactive");
    return this.readingHandler;
  }

  changeScreenOrientation(angle: number | null): void {
    this.screenAngle = angle;
    this.screenHandler?.(angle);
  }
}

class FakeRenderer implements RendererAdapter {
  stereoProfile: StereoPresentationProfile | null = null;
  readonly frames: BackendFrame[] = [];
  private frameCallback: Parameters<RendererAdapter["startLoop"]>[0] | null = null;

  startLoop(callback: Parameters<RendererAdapter["startLoop"]>[0]): void { this.frameCallback = callback; }
  stopLoop(): void { this.frameCallback = null; }
  render(frame: BackendFrame): void { this.frames.push(frame); }
  setFlightPose(): void {}
  setLakeVisualCondition(): void {}
  setFlightCameraMode(): void {}
  setCinematicCameraView(): void {}
  transformTrackingPose(pose: Parameters<RendererAdapter["transformTrackingPose"]>[0]): typeof pose { return pose; }
  resize(): void {}
  setStereoPresentation(profile: StereoPresentationProfile | null): void { this.stereoProfile = profile; }
  setSelectRayHandler(): void {}
  dispose(): void {}

  tick(timestampMs: number): void { this.frameCallback?.(timestampMs, null); }
}

class FakePhoneVrGamepad implements PhoneVrGamepadInputPort {
  state = gamepadState([0, 0, 0, 0], [false, false]);

  readState() {
    return this.state;
  }
}

function createBackend(
  sensors: FakePhoneVrSensors,
  renderer: FakeRenderer,
  unavailable: string[] = [],
  firstSampleTimeoutMs = 3000
): PhoneVrPresentationBackend {
  return new PhoneVrPresentationBackend(
    sensors, renderer, viewport, () => undefined,
    (message) => { unavailable.push(message); },
    { opticalProfile: PHONE_VR_OPTICAL_PROFILE, firstSampleTimeoutMs, nowMs: () => sensors.currentTimeMs }
  );
}

function withMenuAnchors(viewModel: UiViewModel): UiViewModel {
  return Object.freeze({
    ...viewModel,
    panels: Object.freeze(viewModel.panels.map((panel) => Object.freeze({ ...panel, anchor: "menu" as const })))
  });
}

function viewport(): ViewportSize {
  return Object.freeze({ x: 1280, y: 800, pixelRatio: 1 });
}

function reading(alpha: number, beta: number, gamma: number, timestampMs = 100): PhoneVrSensorReading {
  return Object.freeze({ alpha, beta, gamma, timestampMs });
}

function gamepadState(axes: readonly number[], buttons: readonly boolean[]) {
  return Object.freeze({ axes, buttons });
}

function expectScrollAction(actions: readonly UiAction[]): void {
  const scrollAction = actions.find((action) => action.type === "scroll");
  expect(scrollAction?.type).toBe("scroll");
  if (scrollAction?.type === "scroll") {
    expect(scrollAction.deltaX).toBe(0);
    expect(scrollAction.deltaY).toBeCloseTo(4.32);
  }
}

function axisQuaternion(axis: "x" | "y" | "z", degrees: number) {
  const halfAngle = degrees * Math.PI / 360;
  const sine = Math.sin(halfAngle);
  return quaternion(
    Math.cos(halfAngle),
    axis === "x" ? sine : 0,
    axis === "y" ? sine : 0,
    axis === "z" ? sine : 0
  );
}

function createMockBrowserWindow(options: {
  readonly secure?: boolean;
  readonly orientationApi?: boolean;
  readonly screenOrientationApi?: boolean;
} = {}) {
  const windowListeners = new Map<string, (event: unknown) => void>();
  const orientationListeners = new Map<string, () => void>();
  const permissionRequests: boolean[] = [];
  let resolveOrientationListener: (() => void) | null = null;
  const orientationListening = new Promise<void>((resolve) => { resolveOrientationListener = resolve; });
  let screenAngle = 0;
  const orientationConstructor = options.orientationApi === false ? undefined : Object.assign(
    function MockDeviceOrientationEvent() {},
    {
      requestPermission: (absolute?: boolean): Promise<"granted" | "denied" | "prompt"> => {
        permissionRequests.push(absolute ?? false);
        return Promise.resolve("granted");
      }
    }
  );
  const screenOrientation = options.screenOrientationApi === false ? undefined : {
    get angle() { return screenAngle; },
    addEventListener: (type: string, listener: () => void) => { orientationListeners.set(type, listener); },
    removeEventListener: (type: string) => { orientationListeners.delete(type); }
  };
  const fakeWindow = {
    isSecureContext: options.secure ?? true,
    DeviceOrientationEvent: orientationConstructor,
    screen: { orientation: screenOrientation },
    addEventListener: (type: string, listener: unknown) => {
      windowListeners.set(type, listener as (event: unknown) => void);
      if (type === "deviceorientation") resolveOrientationListener?.();
    },
    removeEventListener: (type: string) => { windowListeners.delete(type); }
  };
  return {
    window: fakeWindow as Parameters<typeof createBrowserPhoneVrSensorPort>[0],
    permissionRequests,
    orientationListening,
    emitDeviceOrientation(value: PhoneVrSensorReading) {
      windowListeners.get("deviceorientation")?.({
        alpha: value.alpha, beta: value.beta, gamma: value.gamma, timeStamp: value.timestampMs
      });
    },
    emitScreenOrientationChange() { orientationListeners.get("change")?.(); },
    setScreenAngle(value: number) { screenAngle = value; }
  };
}
